import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { chromium, webkit } from "playwright";

const [svgPath, screenshotPath, browserName = "chromium"] = process.argv.slice(2);
if (!svgPath || !screenshotPath) {
  console.error("usage: svg_browser_probe.mjs <input.svg> <screenshot.png> [chromium|webkit]");
  process.exit(2);
}
const browserType = { chromium, webkit }[browserName];
if (!browserType) {
  console.error(`unsupported browser ${browserName}`);
  process.exit(2);
}

const svg = fs.readFileSync(svgPath, "utf8").replace(/<\?xml[^>]*>\s*/i, "");
const browser = await browserType.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 820, height: 820 } });
  const browserLogs = [];
  page.on("console", (message) => browserLogs.push(`${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => browserLogs.push(`pageerror: ${error.message}`));
  await page.setContent(`<!doctype html>
    <style>
      html, body { margin: 0; background: white; }
      #mount { width: 800px; height: 800px; padding: 10px; box-sizing: border-box; }
      #mount > svg { width: 100%; height: 100%; preserve-aspect-ratio: xMidYMid meet; }
    </style>
    <div id="mount">${svg}</div>`);
  await page.evaluate(() => document.fonts.ready);
  const facts = await page.locator("#mount > svg").evaluate((root) => {
    const rootRect = root.getBoundingClientRect();
    const visible = (element) => {
      const style = getComputedStyle(element);
      return style.display !== "none" && style.visibility !== "hidden"
        && Number.parseFloat(style.opacity || "1") > 0;
    };
    const relativeBounds = (rect) => {
      const epsilon = 0.5;
      return {
        x_px: rect.left - rootRect.left,
        y_px: rect.top - rootRect.top,
        right_px: rect.right - rootRect.left,
        bottom_px: rect.bottom - rootRect.top,
        width_px: rect.width,
        height_px: rect.height,
        intersects_viewport: rect.right > rootRect.left && rect.bottom > rootRect.top
          && rect.left < rootRect.right && rect.top < rootRect.bottom,
        clipped: rect.left < rootRect.left - epsilon || rect.top < rootRect.top - epsilon
          || rect.right > rootRect.right + epsilon || rect.bottom > rootRect.bottom + epsilon,
        normalized: {
          x: (rect.left - rootRect.left) / rootRect.width,
          y: (rect.top - rootRect.top) / rootRect.height,
          right: (rect.right - rootRect.left) / rootRect.width,
          bottom: (rect.bottom - rootRect.top) / rootRect.height,
        },
      };
    };
    const logicalBounds = (element) => {
      const bounds = element.getBBox();
      const elementToScreen = element.getScreenCTM();
      const rootToScreen = root.getScreenCTM();
      if (!elementToScreen || !rootToScreen) {
        return null;
      }
      const screenToRoot = rootToScreen.inverse();
      const points = [
        [bounds.x, bounds.y],
        [bounds.x + bounds.width, bounds.y],
        [bounds.x + bounds.width, bounds.y + bounds.height],
        [bounds.x, bounds.y + bounds.height],
      ].map(([x, y]) => new DOMPoint(x, y).matrixTransform(elementToScreen).matrixTransform(screenToRoot));
      const xs = points.map((point) => point.x);
      const ys = points.map((point) => point.y);
      const x = Math.min(...xs);
      const y = Math.min(...ys);
      const right = Math.max(...xs);
      const bottom = Math.max(...ys);
      const viewBox = root.viewBox.baseVal;
      const epsilon = Math.max(viewBox.width, viewBox.height) * 1e-9 + 1e-9;
      return {
        logical_x: x,
        logical_y: y,
        logical_right: right,
        logical_bottom: bottom,
        intersects_view_box: right > viewBox.x && bottom > viewBox.y
          && x < viewBox.x + viewBox.width && y < viewBox.y + viewBox.height,
        clipped_view_box: x < viewBox.x - epsilon || y < viewBox.y - epsilon
          || right > viewBox.x + viewBox.width + epsilon
          || bottom > viewBox.y + viewBox.height + epsilon,
      };
    };
    const painted = [...root.querySelectorAll(
      "path,line,polyline,polygon,rect,circle,ellipse,text,image"
    )].filter((element) => visible(element)).map((element) => {
      const rect = element.getBoundingClientRect();
      return {
        tag: element.tagName.toLowerCase(),
        ...relativeBounds(rect),
        ...logicalBounds(element),
      };
    }).filter((item) => item.width_px > 0 || item.height_px > 0);
    const texts = [...root.querySelectorAll("text")].map((element) => {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return {
        text: element.textContent,
        font_size_px: Number.parseFloat(style.fontSize),
        ...relativeBounds(rect),
        ...logicalBounds(element),
        visible: visible(element),
      };
    });
    return {
      width: root.getAttribute("width"),
      height: root.getAttribute("height"),
      view_box: root.getAttribute("viewBox"),
      rendered_width_px: rootRect.width,
      rendered_height_px: rootRect.height,
      painted_element_count: painted.length,
      painted_text_count: texts.filter((item) => item.visible && item.width_px > 0 && item.height_px > 0).length,
      max_font_size_px: texts.reduce((maximum, item) => Math.max(maximum, item.font_size_px || 0), 0),
      user_agent: navigator.userAgent,
      painted,
      texts,
    };
  });
  facts.browser_name = browserName;
  facts.browser_logs = browserLogs;
  facts.image_embedding = await page.locator("#mount > svg").evaluate(async (root) => {
    const source = new XMLSerializer().serializeToString(root);
    const url = URL.createObjectURL(new Blob([source], { type: "image/svg+xml" }));
    try {
      const image = new Image();
      image.style.width = "800px";
      image.style.height = "800px";
      image.style.objectFit = "contain";
      image.src = url;
      document.body.append(image);
      await image.decode();
      const rect = image.getBoundingClientRect();
      return {
        rendered_width_px: rect.width,
        rendered_height_px: rect.height,
        natural_width_px: image.naturalWidth,
        natural_height_px: image.naturalHeight,
      };
    } finally {
      URL.revokeObjectURL(url);
    }
  });
  facts.raster = await page.locator("#mount > svg").evaluate(async (root) => {
    const source = new XMLSerializer().serializeToString(root);
    const url = URL.createObjectURL(new Blob([source], { type: "image/svg+xml" }));
    try {
      const image = new Image();
      image.src = url;
      await image.decode();
      const canvas = document.createElement("canvas");
      canvas.width = 400;
      canvas.height = 400;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      context.fillStyle = "#ffffff";
      context.fillRect(0, 0, canvas.width, canvas.height);
      context.drawImage(image, 0, 0, canvas.width, canvas.height);
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      let nonWhitePixels = 0;
      let minX = canvas.width;
      let minY = canvas.height;
      let maxX = -1;
      let maxY = -1;
      for (let index = 0; index < pixels.length; index += 4) {
        if (pixels[index] < 245 || pixels[index + 1] < 245 || pixels[index + 2] < 245) {
          nonWhitePixels += 1;
          const pixel = index / 4;
          const x = pixel % canvas.width;
          const y = Math.floor(pixel / canvas.width);
          minX = Math.min(minX, x);
          minY = Math.min(minY, y);
          maxX = Math.max(maxX, x);
          maxY = Math.max(maxY, y);
        }
      }
      return {
        width_px: canvas.width,
        height_px: canvas.height,
        non_white_pixels: nonWhitePixels,
        occupancy: nonWhitePixels / (canvas.width * canvas.height),
        painted_bounds: nonWhitePixels === 0 ? null : {
          min_x_px: minX,
          min_y_px: minY,
          max_x_px: maxX,
          max_y_px: maxY,
        },
      };
    } finally {
      URL.revokeObjectURL(url);
    }
  });
  fs.mkdirSync(path.dirname(screenshotPath), { recursive: true });
  await page.locator("#mount").screenshot({ path: screenshotPath });
  process.stdout.write(`${JSON.stringify(facts)}\n`);
} finally {
  await browser.close();
}

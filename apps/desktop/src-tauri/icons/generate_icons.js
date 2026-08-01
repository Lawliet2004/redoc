const fs = require('fs');
const path = require('path');

const iconsDir = __dirname;
if (!fs.existsSync(iconsDir)) {
  fs.mkdirSync(iconsDir, { recursive: true });
}

// Minimal 1x1 valid PNG bytes
const pngBuffer = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64'
);

// Minimal ICO header + PNG payload
const icoHeader = Buffer.from([
  0x00, 0x00, // Reserved
  0x01, 0x00, // Type 1 = ICO
  0x01, 0x00, // 1 image
  0x01, 0x01, // Width, Height (1x1)
  0x00, 0x00, // Colors
  0x00, 0x00, // Reserved
  0x00, 0x00, // Color planes
  0x20, 0x00, // 32 bpp
  ...[pngBuffer.length & 0xff, (pngBuffer.length >> 8) & 0xff, (pngBuffer.length >> 16) & 0xff, (pngBuffer.length >> 24) & 0xff], // Size
  0x16, 0x00, 0x00, 0x00 // Offset 22
]);
const icoBuffer = Buffer.concat([icoHeader, pngBuffer]);

fs.writeFileSync(path.join(iconsDir, '32x32.png'), pngBuffer);
fs.writeFileSync(path.join(iconsDir, '128x128.png'), pngBuffer);
fs.writeFileSync(path.join(iconsDir, '128x128@2x.png'), pngBuffer);
fs.writeFileSync(path.join(iconsDir, 'icon.png'), pngBuffer);
fs.writeFileSync(path.join(iconsDir, 'icon.icns'), pngBuffer);
fs.writeFileSync(path.join(iconsDir, 'icon.ico'), icoBuffer);

console.log('Icons generated successfully.');

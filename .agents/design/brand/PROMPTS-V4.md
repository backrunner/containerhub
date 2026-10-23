# ContainerHub v4 图标生成记录

日期：2026-10-22。使用内置 image_gen，非 CLI。方向：等距容器堆叠，参考 [Apple Icon Composer](https://developer.apple.com/icon-composer/) 的分层、折射与高光材质语言。最终为静态 PNG 设计稿，不是 Icon Composer 分层文件。

## 初稿

```text
Use case: stylized-concept.
Asset type: one finished macOS application icon for ContainerHub, a native container management tool.
Primary request: design an elegant, highly polished isometric icon whose material quality belongs beside Apple's macOS 27 built-in app icons. This is a new design, not an existing logo recolor.
Subject: a compact architectural stack of exactly THREE substantial rounded rectangular container modules, two side-by-side on the lower level and one centered across the upper level. The modules are solid coherent three-dimensional cuboids, about twice as wide as tall, carefully nested into a single memorable near-cubic silhouette. Small deliberate gaps separate them. The whole assembly is seen in true orthographic isometric projection: visible top and two side faces, vertical edges vertical, receding horizontal edges at 30 degrees, no perspective convergence. The modules evoke beautifully machined modular compute containers. Each visible long face can have just TWO subtle broad inset vertical flutes to hint at container structure; never busy corrugation.
Composition: square icon canvas. A frontal pearl-white softly rounded macOS squircle tile occupies about 92 percent of the canvas, centered with even margin. Only the blue container assembly is isometric; the outer tile faces the viewer straight on. The assembly fills about 68 percent of the canvas width, optically centered, with generous negative space, no overlap beyond the tile. Soft internal contact shadows anchor the object gently to the tile.
Materials and finish: cobalt-blue satin glass cores with restrained translucent bevels, frosted icy-blue top surfaces, rich deep cobalt right faces, softly milled rounded corners, beautifully controlled narrow edge highlights. Real volume and material depth, not a flat vector extrusion. Restrained modern Apple Liquid Glass craft: a soft vertical overhead studio light, sharper luminous top edges, slightly darkened side edges, delicate refraction only at the bevels, subtle ambient occlusion between layers. The base is premium porcelain-white frosted glass with a delicate glass rim and very subtle cool-grey depth. Calm, clean, beautifully sculpted. No plastic toy look, no excessive candy gloss, no visual noise.
Palette: cobalt #2463EB, sapphire #1746A2, icy blue, pearl white. No green, violet, rainbow or warm gold.
Background: genuine alpha transparency OUTSIDE the white squircle and its very restrained external shadow. White opaque squircle interior. No checkerboard baked into image.
Constraints: deliver ONE large finished icon only, front-on outer tile with ISOMETRIC subject. Perfect crisp contours and antialiasing, smooth clean surfaces. No text, no letters, no monogram, no badges, no status LEDs, no terminal prompt, no whale, no network nodes, no star or asterisk center, no arrows, no extra base/pedestal, no detached floating cubes, no presentation board, no multiple variants, no caption, no watermark. This must be immediately readable at Dock scale, not a miniature detailed illustration.
```

## 材质与留白精修

```text
Use case: precise-object-edit.
The input is the EDIT TARGET: ContainerHub macOS icon, three blue glass container modules in isometric stack on a pearl-white squircle.
Keep the exact subject identity, three-module arrangement, isometric orientation, blue-and-white palette and straight-on squircle. Refine this into a clean restrained final app icon.
1. CRITICAL edge cleanup: Remove absolutely ALL scattered white/blue specks, rough noise, cyan streaks, fragments, clouds and glow outside the squircle. The entire outer area must be perfectly clean alpha transparency. Make the squircle silhouette mathematically smooth and continuous, with precise antialiased edges. No outer drop shadow at all, so the edge is unambiguous.
2. Shrink ONLY the three-module assembly by approximately 12 percent about its center, keeping the squircle the same size, to provide generous even white breathing room.
3. Reduce the strong white chrome-like highlights on the blue modules by about half. Use quiet satin-frosted blue glass faces, delicate narrow highlights along the top bevels, soft overhead illumination, and subtle contact shadows between containers. Keep clear three-dimensional volume and the beautiful rich blue side faces. White tile: softly frosted pearl surface with a very thin quiet rim, not a thick glossy plastic frame. Make faces smooth, not grainy.
Output ONE centered complete macOS icon on genuine alpha outside the squircle. No new objects, no changes to perspective, no text, no labels, no presentation or checkerboard. Preserve distinct container gaps and crisp geometry.
```

## 最终方形母图

透明外缘的两个探索稿存在杂点，未采用。最终使用不透明方形母图，原型通过 CSS 统一裁切圆角。

```text
Use case: precise-object-edit.
Convert the reference icon into a clean square app-icon MASTER artwork. Keep the exact beautiful three blue frosted glass container modules, their arrangement, their current scale, their isometric perspective, fluted sides and blue shading.
CHANGE ONLY THE BACKGROUND AND OUTER ENCLOSURE:
Remove the existing rounded-square rim, the outside margin, all rough alpha fragments, all cyan speckles and every trace of exterior transparency. Replace ALL background with a single continuous opaque pearl-white square extending to all four exact image edges. This is a full-bleed square asset; the operating system will supply the rounded-square mask. Do NOT draw a rounded square within the image. There must be NO visible boundary or edge of a tile, NO frame and NO external shadow. White meets the four canvas edges seamlessly, with a very delicate cool white gradient from top (#FFFFFF) to bottom (#EAF1F8). Preserve subtle contact shadows immediately behind the modules.
The three containers remain centered and fully contained, with generous white space on all sides. Keep the subject size and placement as in the reference. Satin glass surfaces, delicate top highlights. Do not add text, letters, badges or any extra objects.
Output one perfectly square OPAQUE master image, with all four corners solid pearl white. No alpha, no checkerboard, no rough edges, no floating tile.
```

最终资产：[containerhub-app-icon-v4.png](containerhub-app-icon-v4.png)，1254 × 1254，不透明 PNG。旧版本不覆盖。

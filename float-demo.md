# Float image smoke test

This slide demonstrates a raster image floated beside text. The image should sit on the right while these lines use the remaining space, then the paragraph should return to the full width below the image.

<!-- mdterm:wrap width=32% side=right -->
![Enable Sustainable Growth](screenshots/EnableSustainableGrowth.jpg)

A float keeps the image aligned to the chosen side instead of placing it on its own full-width row. This paragraph is deliberately long enough to continue after the bottom edge of the image. While the image is visible, the text column should be narrower; once the image ends, these words should use all available columns again. Resize the terminal or scroll to inspect how the placement follows the viewport.

---

## Left-side float

<!-- mdterm:wrap width=28% side=left -->
![Bonfile](screenshots/Bonfire.png)

This second slide checks the opposite placement. The text should begin to the right of the image, wrap in the free column, and continue across the full slide width below the image. The same Markdown file can be used to inspect the TUI and generate standalone slide exports.

---

## Left-side float 2

<!-- mdterm:wrap width=15% side=left -->
![Face](screenshots/face.png)

This second slide checks the opposite placement. The text should begin to the right of the image, wrap in the free column, and continue across the full slide width below the image. The same Markdown file can be used to inspect the TUI and generate standalone slide exports.

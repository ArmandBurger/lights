# Third-party notices

`lights` is licensed under the GNU General Public License v3 or later (see
[LICENSE](LICENSE)). It includes the third-party material listed below,
which remains under its own license.

## Fluent UI System Icons

- Source: <https://github.com/microsoft/fluentui-system-icons>
- License: MIT
- Copyright (c) 2020 Microsoft Corporation
- Files (in `crates/lights-gui/assets/icons/hicolor/scalable/actions/`,
  renamed with a `-symbolic` suffix, and compiled into the `lights`
  binary):
  - `ic_fluent_add_24_filled-symbolic.svg`
  - `ic_fluent_subtract_24_filled-symbolic.svg`
  - `ic_fluent_brightness_high_24_filled-symbolic.svg`
  - `ic_fluent_brightness_low_24_filled-symbolic.svg`
  - `ic_fluent_link_24_filled-symbolic.svg`
  - `ic_fluent_link_dismiss_24_filled-symbolic.svg`
- Modifications: fill colour changed from `#212121` to `#ffffff`; files
  renamed. The UI recolours them to match the desktop theme.
- The app icon,
  `crates/lights-gui/assets/icons/hicolor/scalable/apps/io.github.armandburger.lights.svg`,
  is `ic_fluent_lightbulb_person_24_filled.svg` in white, scaled and placed
  on an amber rounded square. It is compiled into the binary and installed
  by the `.deb`.

`lights` is not made, affiliated with or endorsed by Microsoft.

```
MIT License

Copyright (c) 2020 Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

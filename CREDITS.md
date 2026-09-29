# Credits

## Textures

RustCraft's built-in textures (128x128) are its own: they are drawn by the generator in
`tools/texgen/` (`python tools/texgen/build.py` writes the `builtin/rustcraft` pack that the
game embeds).

Minecraft Java resource packs put in the `resourcepacks/` folder can be layered over them
(Options → Graphics → Resource Packs).

## Sounds

Most sounds are synthesized by the game (`src/audio/synth.rs`). The guns' recorded sounds
(`src/audio/samples/`):

- The pistol's and revolver's shots, the silenced shot (a suppressed De Lisle carbine), the
  pistol's magazine, slide and dry fire, and the revolver's cylinder: **Sonniss GDC Game Audio Bundles** (Pole Position Production firearm
  recordings), royalty-free, <https://sonniss.com/gameaudiogdc>.
- The AK's shot: **The Free Firearm Sound Library** (CC0). The AK's magazine and bolt, and the
  revolver's speedloader and single rounds: **OpenGameArt** reload recordings (CC0).

## Animations

The player model's burning and torch-holding poses and its limb smoothing are ported from
**Not Enough Animations** by tr7zw: <https://github.com/tr7zw/NotEnoughAnimations>.
The first-person body option follows **First Person Model** by tr7zw:
<https://github.com/tr7zw/FirstPersonModel>. Both are under the tr7zw Protective License:

> Copyright (c) tr7zw, 2021
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software
> (in source or binary form) and associated documentation files (the "Software"), to use, modify
> and compile the Software, subject to the following conditions:
>
> The Software may not be used to get a) a commercial advantage, or b) monetary compensation.
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
> BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

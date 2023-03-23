# Snip & AutoSave

Automatically save screenshots taken with Snip & Sketch.

## Usage

Simply download the latest release (.exe) from the
[releases page](https://github.com/carlalbrecht/snip-and-autosave/releases),
then run the program.

The program displays a notification area icon, which you can right-click in
order to configure the program. Double-clicking the icon opens the folder that
screenshots are saved in.

## How does this work?

When Snip & Sketch captures a screenshot, it also copies it to the clipboard.
This program works by registering a clipboard listener, which is notified every
time an item is added to the clipboard.

For each item added to the clipboard, a set of
[heuristics](https://github.com/carlalbrecht/snip-and-autosave/blob/master/src/heuristics.rs)
are applied to determine whether Snip & Sketch added the item to the clipboard
or not. 

## Status

The version of Snip & Sketch included in Windows 11 now automatically saves
screenshots (to the same directory that this app did by default -
`%userprofile%/Pictures/Screenshots`), so this app is not needed any more:

![Screenshot 2023-03-23 130755](https://user-images.githubusercontent.com/3380125/227241572-5a84a929-1e7a-4d62-90cf-edf475598f0b.png)

Windows 10 users may still find utility in this app though, since it still ships
with an older version of Snip & Sketch.

If you run this on Windows 11, it still works, but the screenshot is saved twice.

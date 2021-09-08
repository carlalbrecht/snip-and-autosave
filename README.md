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

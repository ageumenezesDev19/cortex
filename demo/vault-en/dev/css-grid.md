# Grid and alignment

## When grid, when flex

Grid is for two dimensions, flex is for one. Writing `flex-wrap` and working out
widths by hand usually means grid was the answer.

## Columns that fit themselves

`repeat(auto-fit, minmax(220px, 1fr))` makes as many columns as will fit, with
no media query at all. It is the trick that saves the most CSS.

## Spacing

`gap` rather than margins on the children. A margin on the last item leaves a
space that someone will later try to remove with a special selector.

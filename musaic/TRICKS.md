# Tricks and outputs

A trick is reusable tile code. Save a pattern as a trick, then place its named tile
from the library wherever you need that pattern. Editing the source tiles updates
all instances. Select a trick instance and choose **Edit source tiles** to return
to the original code. This action works with Tab/Enter as well as pointer input,
selects the source tile, and frames its board location.

**Find linked uses** lists every placed tile that directly uses that saved trick,
including tiles inside nested patterns. Choose a result to navigate to it; Previous
and Next page through larger lists. Source containers and their owned notes show
**Shared source for …** before the editing controls. Duplicating a linked tile keeps
its link; it does not make independent source code. The list follows accepted edits
and Undo, and closes when a project is replaced. It counts authored tiles, not
expanded repeats or downstream audible events.

Choose **Duplicate as independent variation…** from the linked tile's inspector,
Edit menu or command search to create separate source code. The review counts all
copied tiles and definitions, including containing patterns, upstream controls and
nested linked code. Each copied definition has a new name and identity; links
within the variation refer to those copies. The original definitions are retained.
All copied root groups appear together below the existing board and are selected
for editing. Internal connections and sound settings are retained. Outputs outside
the copied groups are not connected automatically; connect the variation when you
want to hear it. A project or selection change invalidates the review until it is
refreshed. One Undo removes the whole variation, and Redo restores its identities.

To make a function, select the final processing tile of a connected snippet.
Under **Save reusable trick**, choose which upstream pattern is its input and
enter a name. Each instance takes its own pattern through its main input. The
source pattern is replaced only for that call. Instances can feed other tricks;
recursive definitions are rejected. Saving with no input produces a reusable
pattern instead. Neither defining nor placing a trick consumes a channel.

An **Instrument** tile chooses the sound for its incoming notes. Connect sound
controls such as Gain, Pan, Gate, Fast, filters, or effects where they belong in
the tile program. Multiple instrument branches may feed one output.

An **Output** only names a timeline lane. Its inspector has a name field; leave it
blank to derive the lane name from the connected instrument. Renaming changes no
notes, timing, sound assignment, or controls. Outputs cannot select tricks,
replace their input patterns, or apply mixing or sound parameters.

**Channels** in the project overview limits the total number of output tiles.
New projects default to 16. Creation, paste/duplicate, save, and import enforce the
limit. Lowering it below the number of existing outputs is rejected. Names,
trick definitions, channel changes, and tile edits support undo/redo.

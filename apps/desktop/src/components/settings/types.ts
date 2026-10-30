// Shapes shared by the Settings building blocks.

/** Ids a row hands to its control, for labels and aria. */
export interface RowIds {
  /** The control's id, for `<label for>`. */
  control: string;
  /** The label's id, for `aria-labelledby`. */
  label: string;
  /** The hint's id, for `aria-describedby`. */
  hint: string;
}

/** One option of a Segmented strip. */
export interface SegmentOption<V extends string> {
  id: V;
  label: string;
  /** Shown as the tooltip. */
  hint?: string;
  disabled?: boolean;
}

/** One option of a ChoiceList. */
export interface Choice<V extends string> {
  id: V;
  label: string;
  hint?: string;
  disabled?: boolean;
  /** Said under the hint while it is disabled. */
  disabledReason?: string;
}

/** One card of an OptionGallery. */
export interface GalleryOption<V extends string> {
  id: V;
  label: string;
  hint?: string;
  disabled?: boolean;
}

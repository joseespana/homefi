// How many slow backend calls are in flight (search, Tidal, library scans, ...):
// drives the progress bar along the top of the window.
export const busy = $state({ count: 0 });

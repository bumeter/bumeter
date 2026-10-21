// Quota-weighted estimates shared by several screens.

/**
 * Upper bound of what unused always-on skills cost over a range: written to the prompt cache once per
 * session (×1.25), read back on every call (×0.1). Same formula as `bum_skills::report::unused_weight`.
 */
export const unusedWeight = (alwaysOn: number, sessions: number, calls: number) => Math.floor((alwaysOn * (sessions * 125 + calls * 10)) / 100);

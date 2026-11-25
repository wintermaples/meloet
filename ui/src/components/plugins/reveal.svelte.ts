/**
 * When 「プラグインの場所…」 in the stopped-plugins dialog opens the sound settings, scroll the plugin locations into view.
 * Only the screen holds this (core does not need to know how far to scroll). A state, so plugin locations already shown hear it too.
 */
export const placesReveal = $state({ pending: false });

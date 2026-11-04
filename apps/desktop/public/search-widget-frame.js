// Runs inside the sandboxed iframe that shows Google's Search Suggestions (SearchSuggestions.svelte).
// The frame has no same-origin access and can't navigate or open windows, so a click on a
// suggestion is handed to the app, which opens it in the browser after checking the address.
// It also reports its height, so the app can size the frame.
(function () {
  document.addEventListener("click", function (e) {
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (!a) return;
    e.preventDefault();
    parent.postMessage({ homefiOpen: a.href }, "*");
  });
  function reportHeight() {
    parent.postMessage({ homefiHeight: document.documentElement.scrollHeight }, "*");
  }
  window.addEventListener("load", reportHeight);
  window.addEventListener("resize", reportHeight);
})();

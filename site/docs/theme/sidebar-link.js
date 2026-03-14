(function () {
  var right = document.querySelector(".right-buttons");
  if (!right) return;
  var link = document.createElement("a");
  link.href = "/";
  link.title = "Back to site";
  link.setAttribute("aria-label", "Back to site");
  var span = document.createElement("span");
  span.className = "fa-svg";
  span.innerHTML =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 576 512"><path d="M575.8 255.5c0 18-15 32.1-32 32.1h-32l.7 160.2c0 2.7-.2 5.4-.5 8.1V472c0 22.1-17.9 40-40 40H456c-1.1 0-2.2 0-3.3-.1c-1.4 .1-2.8 .1-4.2 .1H416 392c-22.1 0-40-17.9-40-40V400 336c0-26.5-21.5-48-48-48H272c-26.5 0-48 21.5-48 48v64 72c0 22.1-17.9 40-40 40H160 128.1c-1.5 0-3-.1-4.5-.2c-1.2 .1-2.4 .2-3.6 .2H104c-22.1 0-40-17.9-40-40v-72c0-.7 0-1.4 0-2.1V288 256 245.5L32 256c-17 0-32-14-32-32.1c0-9 3-17 10-24L266.4 8c7-7 15-8 22-8s15 2 21 7L564.8 231.5c8 7 12 15 11 24z"/></svg>';
  link.appendChild(span);
  right.insertBefore(link, right.firstChild);
})();

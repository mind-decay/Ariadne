// Semantic fixture: DOM event listeners (should be filtered by skip list)
// Expected boundaries:
//   Producers: 0
//   Consumers: 0
//   Total: 0 (all DOM events are filtered)

const button = document.getElementById("btn");
button.addEventListener("click", handleClick);
button.addEventListener("submit", handleSubmit);
element.on("focus", handleFocus);
window.addEventListener("load", onLoad);
document.addEventListener("DOMContentLoaded", onReady);
element.addEventListener("scroll", handleScroll);
element.addEventListener("keydown", handleKey);
element.addEventListener("mousedown", handleMouse);

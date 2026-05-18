# PageMD Browser Extension

## Example

Suppose you want to crawl all pages from the Tencent Meeting [Open Platform documentation](https://cloud.tencent.com/document/product/1095/83658)
and convert them to Markdown. You can write hooks like this:


```
// Clean
(function() {
  const sels = ['#document-feedback-container', '.J-relatedArticleLayout'];
  let removed = 0;
  sels.forEach(s =>
    document.querySelectorAll(s).forEach(el => { el.remove(); removed++; })
  );
  return { removed };
})()

// Extract
(function() {
  const el = document.querySelector("div.J-mainContent.responsible.documents-container");
  if (!el) return null;
  const clone = el.cloneNode(true);
  clone.querySelectorAll('nav, .ads').forEach(e => e.remove());
  return {
    title: document.title,
    html: clone.innerHTML
  };
})()

// Navigate
(function() {
  const next = document.querySelector("a.next.J-docDetailPaginationPage");
  if (!next || next.classList.contains('disabled'))
    return { success: false };
  next.click();
  return { success: true };
})()

// Stop
(function(context) {
  if (context.currentUrl === 'https://cloud.tencent.com/document/product/1095/94313')
    return { shouldStop: true, reason: 'Reached target' };
  return { shouldStop: false };
})()
```

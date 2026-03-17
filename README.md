# PageMD

## 使用例子

假设你想将腾讯会议的 [开放平台文档](https://cloud.tencent.com/document/product/1095/83658)
全部爬取并转换为 markdown, 则可以编写如下 Hook:


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

const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../target/site');
const evidence = path.resolve(__dirname, '../target/website-checks');
fs.mkdirSync(evidence, {recursive:true});
(async()=>{
  const browser = await chromium.launch({executablePath:process.env.CHROME_PATH || '/usr/bin/google-chrome',headless:true,args:['--no-sandbox']});
  try {
    const context = await browser.newContext({reducedMotion:'reduce'});
    const page = await context.newPage();
    const errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    const releaseURL='https://api.github.com/repos/Amibiplob/ClipVault/releases/latest';
    const data={tag_name:'v2.2.0',assets:[{name:'clipvault_2.2.0-1_amd64.deb',size:4000000,browser_download_url:'https://github.com/Amibiplob/ClipVault/releases/download/v2.2.0/clipvault_2.2.0-1_amd64.deb'}]};
    await context.route(releaseURL,route=>route.fulfill({json:data}));
    // Route local build assets in-process: no server, open port, or external site.
    await context.route('https://clipvault.test/**', route => {
      const pathname = new URL(route.request().url()).pathname;
      const relative = pathname.replace(/^\/ClipVault\//,'') || 'index.html';
      const file = path.resolve(root,relative);
      if (!file.startsWith(root + path.sep) || !fs.existsSync(file)) return route.fulfill({status:404,body:'Not found'});
      return route.fulfill({path:file,contentType:({'.html':'text/html','.js':'text/javascript','.css':'text/css','.svg':'image/svg+xml','.png':'image/png'})[path.extname(file)] || 'application/octet-stream'});
    });
    const url='https://clipvault.test/ClipVault/';
    await page.goto(url); await page.locator('#release-status').filter({hasText:'Latest stable release'}).waitFor();
    assert.equal(await page.locator('[data-format=deb]').getAttribute('href'),data.assets[0].browser_download_url);
    assert.equal(await page.locator('[data-format=rpm]').getAttribute('href'),null);
    assert((await page.locator('[data-size]').first().innerText()).includes('3.8 MiB'));
    for(const width of [1440,768,390,320]) {
      await page.setViewportSize({width,height:900});
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),`overflow at ${width}`);
      for(const img of await page.locator('img').all()) assert(await img.evaluate(i=>i.complete&&i.naturalWidth>0),'loaded image');
      await page.screenshot({path:path.join(evidence,`light-${width}.png`),fullPage:true});
    }
    await page.setViewportSize({width:1440,height:1000});
    const faq = page.locator('#faq details').first();
    await faq.locator('summary').focus(); await page.keyboard.press('Enter');
    assert.equal(await faq.getAttribute('open'),'');
    await page.keyboard.press('Enter'); assert.equal(await faq.getAttribute('open'),null);
    await page.locator('#theme').selectOption('dark'); await page.reload();
    assert.equal(await page.locator('html').getAttribute('data-theme'),'dark');
    assert.equal(await page.locator('#theme').inputValue(),'dark');
    await page.screenshot({path:path.join(evidence,'dark-1440.png'),fullPage:true});
    await page.locator('#theme').selectOption('system'); await page.emulateMedia({colorScheme:'dark'});
    assert.equal(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor),'rgb(23, 22, 28)');
    assert.equal(await page.evaluate(()=>getComputedStyle(document.documentElement).scrollBehavior),'auto');
    await page.route(releaseURL,route=>route.abort());
    await page.evaluate(()=>localStorage.removeItem('clipvault-release-cache')); await page.reload();
    await page.locator('#release-status').filter({hasText:'unavailable'}).waitFor();
    assert.equal(await page.locator('[data-format=deb]').getAttribute('href'),null);
    for (const width of [1440,320]) {
      // A new page has no restored focus, fragment, or pointer-selected tab position.
      // Do not click the body or focus the link before testing the first Tab.
      const keyboardPage = await context.newPage();
      try {
        keyboardPage.on('pageerror',error=>errors.push(error.message));
        await keyboardPage.setViewportSize({width,height:900});
        await keyboardPage.goto(url);
        assert(await keyboardPage.evaluate(()=>document.hasFocus() && document.activeElement===document.body),'fresh page starts with body focus');
        const skip = keyboardPage.getByRole('link',{name:'Skip to content',exact:true});
        assert.equal(await skip.count(),1);
        assert.equal(await skip.getAttribute('href'),'#main');
        assert(await skip.evaluate(e=>e.getBoundingClientRect().bottom<=0),'skip link is visually hidden before focus');
        await keyboardPage.keyboard.press('Tab');
        assert(await skip.evaluate(e=>e===document.activeElement),'first Tab focuses the Skip to content link');
        assert(await skip.evaluate(e=>{
          const rect=e.getBoundingClientRect(), style=getComputedStyle(e);
          return rect.width>0 && rect.height>0 && rect.top>=0 && rect.left>=0 && rect.bottom<=innerHeight && rect.right<=innerWidth && style.visibility==='visible' && style.opacity==='1' && style.outlineStyle!=='none' && parseFloat(style.outlineWidth)>0;
        }),'focused skip link is fully visible with a focus ring');
        await keyboardPage.screenshot({path:path.join(evidence,`skip-focused-${width}.png`)});
        await keyboardPage.keyboard.press('Enter');
        const main = keyboardPage.getByRole('main');
        assert.equal(new URL(keyboardPage.url()).hash,'#main');
        assert(await main.evaluate(e=>e===document.activeElement),'activating the skip link focuses main content');
        await keyboardPage.keyboard.press('Tab');
        assert(await main.evaluate(e=>e.contains(document.activeElement)),'Tab continues within main content');
      } finally { await keyboardPage.close(); }
    }
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(evidence,'result.json'),JSON.stringify({passed:true,widths:[1440,768,390,320],checks:['project base path','exact release downloads','missing RPM disabled','actual size','images','overflow','FAQ Enter toggle','theme persistence','system dark','reduced motion','offline fallback','keyboard skip and focus','no page errors'],evidence:'Local browser with intercepted GitHub fixtures; not live release or desktop integration proof.'},null,2));
    console.log('Browser checks passed at 1440/768/390/320px; screenshots in target/website-checks. GitHub responses are fixtures.');
    await context.close();
  } finally { await browser.close(); }
})().catch(error=>{fs.writeFileSync(path.join(evidence,'result.json'),JSON.stringify({passed:false,error:String(error),evidence:'Browser run did not complete; no layout or interaction pass is claimed.'},null,2));console.error(error);process.exitCode=1;});

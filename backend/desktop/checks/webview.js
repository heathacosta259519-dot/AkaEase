(async () => {
  const invoke = (...args) => window.__TAURI_INTERNALS__.invoke(...args);
  const checks = [];
  const violations = [];
  document.addEventListener('securitypolicyviolation', event => violations.push(event.violatedDirective));
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const until = async (name, predicate) => {
    for (let i = 0; i < 150; i++) {
      if (await predicate()) { checks.push(name); return; }
      await delay(50);
    }
    throw new Error(name);
  };
  const button = title => document.querySelector(`button[title="${title}"]`);
  const textButton = text => [...document.querySelectorAll('button')].find(el => el.textContent.trim() === text);
  const state = () => invoke('player_snapshot');
  const track = id => ({id, title:`Synthetic ${id}`, artists:[], album:{id:'1',name:'Acceptance',coverUrl:null},durationMs:90000});
  try {
    await until('React rendered in actual WebKit', () => textButton('发现音乐') && document.querySelector('footer'));
    await until('Tailwind CSS loaded', () => Math.round(document.querySelector('footer').getBoundingClientRect().height) === 76);
    // Let initial session restore finish before sending playback commands.
    await delay(300);
    if ((await invoke('session_snapshot')).profile !== null) throw new Error('account isolation');
    textButton('设置').click();
    await until('Settings loaded through real Tauri IPC', () => document.body.textContent.includes('正常 (Ready)'));
    const checkbox = document.querySelector('input[type="checkbox"]');
    checkbox.click();
    await until('Settings checkbox writes backend config', async () => !(await invoke('config_get')).restoreQueue);
    await until('Settings checkbox reflects saved config', () => !checkbox.checked && document.body.textContent.includes('配置已保存'));
    await delay(50);
    checkbox.click();
    await until('Settings change can be reverted', async () => (await invoke('config_get')).restoreQueue);
    textButton('发现音乐').click();
    await invoke('player_replace', {tracks:[track('1'),track('2')],selected:0,autoplay:false});
    await until('Player events update track metadata', () => document.querySelector('footer').textContent.includes('Synthetic 1'));
    button('播放').click();
    await until('UI play controls real GStreamer', async () => (await state()).playback.state === 'playing' && button('暂停'));
    button('暂停').click();
    await until('UI pause controls real GStreamer', async () => (await state()).playback.state === 'paused');
    button('顺序播放').click();
    await until('Repeat UI and backend agree', async () => (await state()).repeat === 'all' && button('列表循环'));
    button('开启随机播放').click();
    await until('Shuffle UI and backend agree', async () => (await state()).shuffle && button('关闭随机播放'));
    button('关闭随机播放').click();
    await until('Shuffle disabled', async () => !(await state()).shuffle);
    button('下一曲').click();
    await until('Next track updates both UI and engine', async () => (await state()).currentIndex === 1 && document.querySelector('footer').textContent.includes('Synthetic 2'));
    button('播放队列').click();
    await until('Queue drawer retrieves backend queue', () => document.body.textContent.includes('当前播放队列') && document.querySelectorAll('button[title="从队列移除"]').length === 2);
    document.querySelectorAll('button[title="从队列移除"]')[0].click();
    await until('Queue deletion reaches backend', async () => (await invoke('player_queue')).tracks.length === 1);
    button('播放队列').click();
    button('展开歌词').click();
    await until('Lyrics view consumes cached backend lyrics', () => document.body.textContent.includes('Synthetic lyric for WebView check'));
    textButton('收起歌词').click();
    await invoke('player_stop');
    if (violations.length) throw new Error(`CSP violations: ${violations.join(',')}`);
    await invoke('plugin:event|emit', {event:'webview-check-result',payload:{ok:true,checks,origin:location.origin,violations}});
  } catch (error) {
    await invoke('plugin:event|emit', {event:'webview-check-result',payload:{ok:false,checks,failed:String(error),origin:location.origin,violations}});
  }
})();

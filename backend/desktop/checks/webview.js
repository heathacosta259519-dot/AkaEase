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
    for (const [command, args] of [
      ['track_like', {trackId:'7',like:true}],
      ['playlist_create', {name:'Synthetic playlist'}],
      ['playlist_delete', {playlistId:'1'}],
      ['playlist_tracks_op', {playlistId:'1',trackIds:['7'],op:'add'}],
      ['playlist_subscribe', {playlistId:'1',subscribe:true}],
    ]) {
      let error;
      try { await invoke(command, args); } catch (result) { error = result; }
      if (error?.code !== 'unauthorized') throw new Error(`${command} did not reach session validation: ${JSON.stringify(error)}`);
      checks.push(`${command} requires login through real Tauri IPC`);
    }
    for (const command of ['music_artist_detail', 'music_album_detail', 'music_artist_songs', 'music_artist_albums']) {
      let error;
      try { await invoke(command, {id:'0',offset:0,limit:20}); } catch (result) { error = result; }
      if (error?.code !== 'invalid_input') throw new Error(`${command} blocked before ID validation: ${JSON.stringify(error)}`);
      checks.push(`${command} reaches backend through real Tauri IPC`);
    }
    textButton('设置').click();
    await until('Settings loaded through real Tauri IPC', () => document.body.textContent.includes('正常 (Ready)'));
    const checkbox = document.querySelector('input[type="checkbox"]');
    checkbox.click();
    await until('Settings checkbox writes backend config', async () => !(await invoke('config_get')).restoreQueue);
    await until('Settings checkbox reflects saved config', () => !checkbox.checked && document.body.textContent.includes('配置已保存'));
    await delay(50);
    checkbox.click();
    await until('Settings change can be reverted', async () => (await invoke('config_get')).restoreQueue);
    const qualityConfig = await invoke('config_get');
    if (qualityConfig.defaultQuality !== 'exhigh') throw new Error('quality default');
    await invoke('config_set', {config:{...qualityConfig,defaultQuality:'hires'}});
    await until('Default quality persists through real Tauri IPC', async () => (await invoke('config_get')).defaultQuality === 'hires');
    await invoke('config_set', {config:qualityConfig});
    textButton('发现音乐').click();
    await invoke('player_replace', {tracks:[track('1'),track('2')],selected:0,autoplay:false});
    await until('Player events update track metadata', () => document.querySelector('footer').textContent.includes('Synthetic 1'));
    button('播放').click();
    await until('UI play controls real GStreamer', async () => (await state()).playback.state === 'playing' && button('暂停'));
    button('暂停').click();
    await until('UI pause controls real GStreamer', async () => (await state()).playback.state === 'paused');
    const beforeQuality = await state();
    await invoke('player_seek', {positionMs:1200,selectionId:beforeQuality.selectionId});
    await invoke('player_set_quality', {quality:'lossless'});
    await until('Quality switch reloads paused GStreamer at the saved position', async () => {
      const s = await state();
      return !s.resolving && s.targetQuality === 'lossless' && s.playback.state === 'paused' && s.playback.positionMs === 1200 && s.queueRevision === beforeQuality.queueRevision && s.selectionId !== beforeQuality.selectionId;
    });
    await until('Actual quality reports a playable downgrade', async () => {
      const s = await state();
      return s.actualQuality === 'standard' && s.actualBitrate === 128000 && s.format === 'wav';
    });
    await invoke('player_set_quality', {quality:'exhigh'});
    await until('Quality switch can be reverted without autoplay', async () => !(await state()).resolving && (await state()).playback.state === 'paused');
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
    await invoke('player_pause');
    await until('Audio paused before background expansion', async () => (await state()).playback.state === 'paused' && button('播放'));
    const beforeExpansion = await state();
    const expandedTracks = [track('2'), ...Array.from({length:2004}, (_, index) => track(String(index + 3)))];
    await invoke('player_expand', {tracks:expandedTracks,revision:beforeExpansion.queueRevision});
    await until('Queue expands to 2005 tracks without changing selection or paused audio', async () => {
      const snapshot = await state();
      return snapshot.queueLength === 2005 && snapshot.selectionId === beforeExpansion.selectionId && snapshot.current.id === '2' && snapshot.playback.state === beforeExpansion.playback.state && snapshot.playback.positionMs === beforeExpansion.playback.positionMs;
    });
    button('播放').click();
    await until('Expanded queue resumes real audio without reselecting', async () => (await state()).playback.state === 'playing' && (await state()).selectionId === beforeExpansion.selectionId);
    textButton('发现音乐').click();
    [...document.querySelectorAll('h3')].find(el => el.textContent === '热歌榜').click();
    await until('Large playlist renders its first 100 songs', () => document.querySelectorAll('.song-table-row').length === 100);
    document.querySelectorAll('.song-table-row')[56].dispatchEvent(new MouseEvent('dblclick', {bubbles:true}));
    await until('Double click starts selected audio before slow background pages complete', async () => {
      const snapshot = await state();
      return snapshot.current?.id === '57' && snapshot.queueLength === 100 && snapshot.playback.state === 'playing';
    });
    const started = await state();
    button('暂停').click();
    await until('Pause remains usable during background loading', async () => (await state()).playback.state === 'paused');
    await until('Background completes all 2005 songs without restarting paused audio', async () => {
      const snapshot = await state();
      return snapshot.queueLength === 2005 && snapshot.current?.id === '57' && snapshot.selectionId === started.selectionId && snapshot.playback.state === 'paused';
    });
    await invoke('player_stop');
    if (violations.length) throw new Error(`CSP violations: ${violations.join(',')}`);
    await invoke('plugin:event|emit', {event:'webview-check-result',payload:{ok:true,checks,origin:location.origin,violations}});
  } catch (error) {
    await invoke('plugin:event|emit', {event:'webview-check-result',payload:{ok:false,checks,failed:String(error),origin:location.origin,violations}});
  }
})();

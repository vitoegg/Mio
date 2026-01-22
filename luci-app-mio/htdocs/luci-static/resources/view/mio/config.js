'use strict';
'require form';
'require view';
'require uci';
'require poll';
'require ui';
'require tools.mio as mio';

const translations = {
    'Unknown': '未知',
    'Running': '运行中',
    'Not Running': '已停止',
    'App Version': '软件版本',
    'Core Version': '内核版本',
    'Plugin Version': '插件版本',
    'Server Status': '服务器状态',
    'Log': '日志',
    'Mio': 'Mio',
    'Shadowsocks-Rust server on OpenWrt, helping you access services on your home network from the public internet.': 'OpenWrt 上的 Shadowsocks-Rust 服务器，帮助您从公网访问家庭网络中的服务。',
    'App Info': '应用信息',
    'App Settings': '应用配置',
    'Server Settings': '服务器配置',
    'Core Control': '内核控制',
    'Enable': '启用',
    'Disable': '停用',
    'Server Port': '服务器端口',
    'Method': '加密方法',
    'Password': '密码',
    'Log Level': '日志等级',
    'Obfs': 'Obfs 选项',
    'HTTP': 'HTTP',
    'TLS': 'TLS',
    'Obfs Host': 'Obfs Host',
    'Mio Log': 'Mio 日志',
    'Refresh': '刷新',
    'Close': '关闭',
    'Loading...': '加载中...',
    'No log data': '暂无相关日志',
    'Unable to load log data: %s': '无法读取日志：%s'
};

const GLOBAL_SECTION = 'global';
const SERVER_SECTION = 'server';

function isZhLanguage() {
    const lang = (typeof L !== 'undefined' && L.env && (L.env.lang || L.env.language || L.env.locale)) ||
        (typeof document !== 'undefined' && document.documentElement && document.documentElement.lang) ||
        (typeof navigator !== 'undefined' && navigator.language) ||
        '';
    return lang.toLowerCase().replace('_', '-').startsWith('zh');
}

function _(msgid) {
    if (!isZhLanguage()) {
        return msgid;
    }
    return translations[msgid] || msgid;
}

function normalizeVersion(value) {
    if (!value) {
        return _('Unknown');
    }

    const trimmed = String(value).trim();
    const match = trimmed.match(/\d+(?:[\w.+~-]*)/);
    return match ? match[0] : trimmed;
}

function renderStatus(running) {
    return updateStatus(E('span', {
        id: 'core_status',
        style: 'font-weight: bold;'
    }), running);
}

function updateStatus(element, running) {
    if (element) {
        element.style.color = running ? 'green' : 'red';
        element.textContent = running ? _('Running') : _('Not Running');
    }
    return element;
}

function loadLogData() {
    return mio.log().then(function (logdata) {
        const filtered = String(logdata || '').trim();
        return filtered || _('No log data');
    });
}

function showLogModal() {
    const textarea = E('textarea', {
        'class': 'cbi-input-textarea',
        'style': 'width: 100%; min-height: 240px; font-size: 12px;',
        'readonly': 'readonly',
        'wrap': 'off'
    });
    const refreshBtn = E('button', { 'class': 'cbi-button cbi-button-neutral' }, _('Refresh'));
    const closeBtn = E('button', { 'class': 'cbi-button cbi-button-neutral' }, _('Close'));

    function setText(text) {
        textarea.value = text;
    }

    function load() {
        setText(_('Loading...'));
        return loadLogData().then(setText).catch(function (err) {
            const msg = (err && err.message) ? err.message : String(err);
            setText(_('Unable to load log data: %s').format(msg));
        });
    }

    refreshBtn.addEventListener('click', function (ev) {
        ev.preventDefault();
        load();
    });

    closeBtn.addEventListener('click', function (ev) {
        ev.preventDefault();
        ui.hideModal();
    });

    ui.showModal(_('Mio Log'), [
        E('div', { 'class': 'cbi-section' }, [textarea]),
        E('div', { 'class': 'cbi-section' }, [refreshBtn, ' ', closeBtn])
    ]);

    load();
}

function renderLogButton() {
    const button = E('button', { 'class': 'cbi-button cbi-button-neutral' }, _('Log'));
    button.addEventListener('click', function (ev) {
        ev.preventDefault();
        showLogModal();
    });
    return button;
}

function renderInfoTable(appVersion, coreVersion, pluginVersion, running) {
    const headerStyle = 'text-align: left; width: 20%;';
    const cellStyle = 'text-align: left; width: 20%;';

    return E('table', { 'class': 'table cbi-section-table', 'style': 'width: 100%; table-layout: fixed;' }, [
        E('tr', {}, [
            E('th', { 'style': headerStyle }, _('App Version')),
            E('th', { 'style': headerStyle }, _('Core Version')),
            E('th', { 'style': headerStyle }, _('Plugin Version')),
            E('th', { 'style': headerStyle }, _('Server Status')),
            E('th', { 'style': headerStyle }, _('Log'))
        ]),
        E('tr', {}, [
            E('td', { 'style': cellStyle }, appVersion),
            E('td', { 'style': cellStyle }, coreVersion),
            E('td', { 'style': cellStyle }, pluginVersion),
            E('td', { 'style': cellStyle }, renderStatus(running)),
            E('td', { 'style': cellStyle }, renderLogButton())
        ])
    ]);
}

return view.extend({
    load: function () {
        return Promise.all([
            uci.load('mio'),
            mio.version(),
            mio.status()
        ]);
    },

    render: function (data) {
        const appVersion = normalizeVersion(data[1].app ?? _('Unknown'));
        const coreVersion = data[1].ssserver ?? _('Unknown');
        const pluginVersion = data[1].obfs ? String(data[1].obfs).trim() : _('Unknown');
        const running = data[2];

        let m, s, o;

        m = new form.Map('mio', _('Mio'),
            _('Shadowsocks-Rust server on OpenWrt, helping you access services on your home network from the public internet.'));

        // Status Section
        s = m.section(form.NamedSection, GLOBAL_SECTION, 'mio', _('App Info'));
        s.anonymous = true;
        s.addremove = false;
        s.render = function () {
            setTimeout(function () {
                poll.add(function () {
                    return L.resolveDefault(mio.status(), false).then(function (running) {
                        updateStatus(document.getElementById('core_status'), running);
                    });
                });
            }, 100);

            return E('div', { 'class': 'cbi-section' }, [
                E('h3', {}, _('App Info')),
                renderInfoTable(appVersion, coreVersion, pluginVersion, running)
            ]);
        };

        // Global Settings
        s = m.section(form.NamedSection, GLOBAL_SECTION, 'mio', _('App Settings'));
        s.anonymous = true;
        s.addremove = false;

        o = s.option(form.ListValue, 'enabled', _('Core Control'));
        o.value('1', _('Enable'));
        o.value('0', _('Disable'));
        o.default = '0';
        o.rmempty = false;

        o = s.option(form.ListValue, 'log_level', _('Log Level'));
        o.value('error', 'error');
        o.value('warn', 'warn');
        o.value('info', 'info');
        o.value('debug', 'debug');
        o.value('trace', 'trace');
        o.default = 'info';
        o.rmempty = false;

        // Server Settings
        s = m.section(form.NamedSection, SERVER_SECTION, 'shadowsocks', _('Server Settings'));
        s.anonymous = true;
        s.addremove = false;

        o = s.option(form.Value, 'server_port', _('Server Port'));
        o.datatype = 'port';
        o.placeholder = '8388';
        o.rmempty = false;

        o = s.option(form.ListValue, 'method', _('Method'));
        o.value('2022-blake3-aes-128-gcm', '2022-blake3-aes-128-gcm');
        o.value('2022-blake3-aes-256-gcm', '2022-blake3-aes-256-gcm');
        o.value('aes-128-gcm', 'aes-128-gcm');
        o.value('aes-256-gcm', 'aes-256-gcm');
        o.value('chacha20-ietf-poly1305', 'chacha20-ietf-poly1305');
        o.rmempty = false;

        o = s.option(form.Value, 'password', _('Password'));
        o.password = true;
        o.rmempty = false;

        o = s.option(form.ListValue, 'obfs_mode', _('Obfs'));
        o.value('disable', _('Disable'));
        o.value('http', _('HTTP'));
        o.value('tls', _('TLS'));
        o.default = 'disable';
        o.rmempty = false;

        o = s.option(form.Value, 'obfs_host', _('Obfs Host'));
        o.depends('obfs_mode', 'http');
        o.depends('obfs_mode', 'tls');
        o.default = 'speedtest.cn';
        o.placeholder = 'speedtest.cn';
        o.rmempty = false;

        this.map = m;
        return m.render();
    }
});

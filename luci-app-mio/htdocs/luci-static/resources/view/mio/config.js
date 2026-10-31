'use strict';
'require form';
'require fs';
'require poll';
'require rpc';
'require uci';
'require ui';
'require view';

const callRCList = rpc.declare({
    object: 'rc',
    method: 'list',
    params: ['name'],
    expect: { '': {} }
});

const callMioVersion = rpc.declare({
    object: 'luci.mio',
    method: 'version',
    expect: { '': {} }
});

// Written by mio in its final format.
const LOG_FILE = '/tmp/log/mio.log';
// Shadowsocks 2022 ciphers take a base64 key of exactly this many bytes.
const SS_KEY_BYTES = {
    '2022-blake3-aes-128-gcm': 16,
    '2022-blake3-aes-256-gcm': 32,
    '2022-blake3-chacha20-poly1305': 32
};
// Only the canonical encoding, which mio requires: the unused bits of the last character are zero.
const SS_KEY = { 16: /^[A-Za-z0-9+/]{21}[AQgw]==$/, 32: /^[A-Za-z0-9+/]{42}[AEIMQUYcgkosw048]=$/ };

function getStatus() {
    return L.resolveDefault(callRCList('mio'), {}).then(function (res) {
        return res.mio?.running ?? null;
    });
}

function updateStatus(element, running) {
    if (element) {
        element.style.color = running == null ? 'gray' : (running ? 'green' : 'red');
        element.textContent = running == null ? _('Unknown') : (running ? _('Running') : _('Not Running'));
    }
    return element;
}

function pollStatus() {
    return getStatus().then(function (running) {
        updateStatus(document.getElementById('core_status'), running);
    });
}

function readLog() {
    return fs.read(LOG_FILE).catch(function (err) {
        // A missing or empty file only means nothing has been logged yet.
        if (err.name == 'NotFoundError' || err.name == 'NoDataError')
            return '';
        throw err;
    });
}

function showLogModal() {
    const textarea = E('textarea', {
        'class': 'cbi-input-textarea',
        'style': 'width: 100%; min-height: 240px; font-family: monospace; font-size: 12px; resize: vertical;',
        'readonly': 'readonly',
        'wrap': 'off'
    });

    function show(text) {
        if (textarea.value == text)
            return;

        // Follow new lines only while the view is already at the bottom.
        const atBottom = textarea.scrollTop + textarea.clientHeight >= textarea.scrollHeight - 1;
        textarea.value = text;
        if (atBottom)
            textarea.scrollTop = textarea.scrollHeight;
    }

    function load() {
        // Stop polling once the modal has been closed.
        if (!textarea.isConnected)
            return poll.remove(load);

        return readLog().then(function (data) {
            show(data.trim() || _('No log data'));
        }).catch(function (err) {
            show(_('Unable to load log data: %s').format(err?.message ?? err));
        });
    }

    function clear() {
        // Truncate instead of deleting: mio keeps the file open.
        return fs.write(LOG_FILE, '').then(load).catch(function (err) {
            textarea.value = _('Unable to clear log data: %s').format(err?.message ?? err);
        });
    }

    ui.showModal(_('Mio Log'), [
        E('div', { 'class': 'cbi-section' }, [textarea]),
        E('div', {}, [
            E('button', { 'class': 'cbi-button cbi-button-negative', 'click': clear }, _('Clear')),
            ' ',
            E('button', { 'class': 'cbi-button cbi-button-neutral', 'click': ui.hideModal }, _('Close'))
        ])
    ]);

    textarea.value = _('Loading...');
    load();
    poll.add(load, 5);
}

function renderInfoSection(version, running) {
    const cellStyle = 'text-align: center;';
    const status = updateStatus(E('span', { 'id': 'core_status', 'style': 'font-weight: bold;' }), running);
    const columns = [
        [_('App Version'), version.app || _('Unknown')],
        [_('Core Version'), version.core || _('Unknown')],
        [_('Running Status'), status],
        [_('Log'), E('button', { 'class': 'cbi-button cbi-button-neutral', 'click': showLogModal }, _('View'))]
    ];

    return E('div', { 'class': 'cbi-section' }, [
        E('h3', {}, _('Status')),
        E('table', { 'class': 'table cbi-section-table', 'style': 'width: 100%; table-layout: fixed;' }, [
            E('tr', {}, columns.map(([title]) => E('th', { 'style': cellStyle }, title))),
            E('tr', {}, columns.map(([, value]) => E('td', { 'style': cellStyle }, value)))
        ])
    ]);
}

function toggle(section, name, title) {
    const o = section.option(form.ListValue, name, title);
    o.value('1', _('Enable'));
    o.value('0', _('Disable'));
    o.default = '0';
    o.rmempty = false;
}

return view.extend({
    load: function () {
        // Load the config together with the info row; form.Map reuses the loaded config.
        return Promise.all([
            L.resolveDefault(callMioVersion(), {}),
            getStatus(),
            uci.load('mio')
        ]);
    },

    render: function ([version, running]) {
        let m, s, o;

        m = new form.Map('mio', _('Mio'),
            _('Shadowsocks server on OpenWrt, helping you access services on your home network from the public internet.'));

        s = m.section(form.NamedSection, 'main', 'mio', _('Settings'));
        s.addremove = false;

        toggle(s, 'enabled', _('Service'));

        o = s.option(form.Value, 'port', _('Listen Port'));
        o.datatype = 'range(1024,65535)';
        o.placeholder = '6160';
        o.rmempty = false;

        o = s.option(form.ListValue, 'cipher', _('Cipher'));
        for (const cipher in SS_KEY_BYTES)
            o.value(cipher, cipher);
        o.default = '2022-blake3-aes-128-gcm';
        o.rmempty = false;

        o = s.option(form.Value, 'password', _('Password'));
        o.password = true;
        o.validate = function (section_id, value) {
            if (this.section.formvalue(section_id, 'enabled') != '1')
                return true;
            if (!value)
                return _('Password is required');

            const bytes = SS_KEY_BYTES[this.section.formvalue(section_id, 'cipher')];
            return SS_KEY[bytes].test(value) || _('The key must be %d bytes in base64, e.g. from: openssl rand -base64 %d').format(bytes, bytes);
        };
        o.renderWidget = function () {
            const node = form.Value.prototype.renderWidget.apply(this, arguments);
            const group = node.querySelector('.control-group');
            const input = node.querySelector('input');

            input?.setAttribute('autocomplete', 'new-password');
            // Overlay the reveal button on the input so the field keeps the same width as the other inputs;
            // keep LuCI's layout if its password widget markup changes.
            if (!group || !input)
                return node;

            group.className = '';
            group.style.cssText = 'position: relative; display: inline-flex;';
            input.style.paddingRight = '2.5rem';
            group.querySelector('button').style.cssText = 'position: absolute; top: 0; right: 0; bottom: 0; margin: 0; border-top-left-radius: 0; border-bottom-left-radius: 0;';
            return node;
        };

        toggle(s, 'open_firewall', _('Allow Through Firewall'));

        o = s.option(form.ListValue, 'log_level', _('Log level'));
        o.value('warning', 'warning');
        o.value('info', 'info');
        o.default = 'warning';
        o.rmempty = false;

        return m.render().then(function (node) {
            node.insertBefore(renderInfoSection(version, running), node.querySelector('.cbi-section'));
            poll.add(pollStatus);
            return node;
        });
    }
});

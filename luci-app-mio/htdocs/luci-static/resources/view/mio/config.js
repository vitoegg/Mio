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

// Written by the mio-log service; logread rotates the current file to .old.
const LOG_FILES = ['/tmp/log/mio.log.old', '/tmp/log/mio.log'];
// logread -F line: "<ctime> <facility>.<priority> <tag>[pid]: <message>". snell-server messages start with
// "<date> <time>.<usec> [<thread>] <<LEVEL>> ", whose level replaces the stream-based syslog priority.
const LOG_LINE = /^\w{3} (\w{3}) +(\d+) (\S+) (\d{4}) \w+\.(\w+) [^:]+: *(?:\S+ \S+ \[[^\]]*\] <(\w+)> )?(.*)$/gm;
const MONTHS = { Jan: '01', Feb: '02', Mar: '03', Apr: '04', May: '05', Jun: '06', Jul: '07', Aug: '08', Sep: '09', Oct: '10', Nov: '11', Dec: '12' };
// snell-server trims config values, so surrounding whitespace would silently change the key.
const PSK_INVALID = /^\s|\s$|[\x00-\x1f\x7f]/;

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

// Formats every recognized line as "<date> <time> [LEVEL] <text>"; other lines are kept as logged.
function formatLog(raw) {
    return raw.replace(LOG_LINE, (line, month, day, time, year, priority, level, text) =>
        `${year}-${MONTHS[month]}-${day.padStart(2, '0')} ${time} [${(level ?? priority).toUpperCase().replace(/^ERR$/, 'ERROR')}] ${text}`).trim();
}

function readLogFile(path) {
    return fs.read(path).catch(function (err) {
        // A missing or empty file only means nothing has been logged yet.
        if (err.name == 'NotFoundError' || err.name == 'NoDataError')
            return '';
        throw err;
    });
}

function readLog() {
    return Promise.all(LOG_FILES.map(readLogFile)).then(function (data) {
        return data.join('');
    });
}

function clearLog() {
    // Truncate instead of deleting: logread keeps the current file open.
    return Promise.all([
        fs.write(LOG_FILES[1], ''),
        fs.remove(LOG_FILES[0]).catch(function (err) {
            if (err.name != 'NotFoundError')
                throw err;
        })
    ]);
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

    let raw = null;

    function load() {
        // Stop polling once the modal has been closed.
        if (!textarea.isConnected)
            return poll.remove(load);

        return readLog().then(function (data) {
            // Only reformat when the log files have changed.
            if (data === raw)
                return;
            raw = data;
            show(formatLog(data) || _('No log data'));
        }).catch(function (err) {
            raw = null;
            show(_('Unable to load log data: %s').format(err?.message ?? err));
        });
    }

    function clear() {
        return clearLog().then(load).catch(function (err) {
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
        [_('Snell Version'), version.snell || _('Unknown')],
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
            _('Snell server on OpenWrt, helping you access services on your home network from the public internet.'));

        s = m.section(form.NamedSection, 'main', 'mio', _('Settings'));
        s.addremove = false;

        toggle(s, 'enabled', _('Snell Service'));

        o = s.option(form.Value, 'server_port', _('Listen Port'));
        o.datatype = 'range(1024,65535)';
        o.placeholder = '6160';
        o.rmempty = false;

        o = s.option(form.Value, 'psk', _('Password'));
        o.password = true;
        o.validate = function (section_id, value) {
            if (this.section.formvalue(section_id, 'enabled') != '1')
                return true;
            if (!value)
                return _('Password is required');

            return !PSK_INVALID.test(value) || _('Password must not have leading or trailing whitespace or control characters.');
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
        o.value('notify', 'notify');
        o.value('info', 'info');
        o.default = 'notify';
        o.rmempty = false;

        return m.render().then(function (node) {
            node.insertBefore(renderInfoSection(version, running), node.querySelector('.cbi-section'));
            poll.add(pollStatus);
            return node;
        });
    }
});

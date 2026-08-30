'use strict';
'require baseclass';
'require rpc';

const callRCList = rpc.declare({
    object: 'rc',
    method: 'list',
    params: ['name'],
    expect: { '': {} }
});

const callRCInit = rpc.declare({
    object: 'rc',
    method: 'init',
    params: ['name', 'action'],
    expect: { '': {} }
});

const callMioVersion = rpc.declare({
    object: 'luci.mio',
    method: 'version',
    expect: { '': {} }
});

const callMioLog = rpc.declare({
    object: 'luci.mio',
    method: 'log',
    expect: { log: '' }
});

return baseclass.extend({
    status: async function () {
        return (await callRCList('mio'))?.mio?.running;
    },

    reload: function () {
        return callRCInit('mio', 'reload');
    },

    restart: function () {
        return callRCInit('mio', 'restart');
    },

    stop: function () {
        return callRCInit('mio', 'stop');
    },

    start: function () {
        return callRCInit('mio', 'start');
    },

    enable: function () {
        return callRCInit('mio', 'enable');
    },

    disable: function () {
        return callRCInit('mio', 'disable');
    },

    version: function () {
        return callMioVersion();
    },

    log: function () {
        return callMioLog().then(function (data) {
            return data || '';
        });
    }
});

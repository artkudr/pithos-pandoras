// Живое обновление страницы: сервер держит счётчик правок базы, страница
// спрашивает его коротким запросом. Долгое соединение не открываем — иначе
// браузер упирается в предел соединений на хост и страницы перестают грузиться.
//
// Если человек что-то печатает, страница не перезагружается — показываем полоску
// с кнопкой, чтобы не потерять введённое.
(function () {
  var NOTICE_INTERVAL = 1500;

  var notice = document.getElementById('live-notice');
  var reload = document.getElementById('live-reload');
  var reloading = false;
  var revision = null;

  function hasDraft() {
    // Открытая форма создания или раскрытый блок — человек работает с ними,
    // перезагрузка закроет форму и собьёт с места.
    if (document.querySelector('.create-row:not([hidden]) form')) {
      return true;
    }
    if (document.querySelector('details[open] form')) {
      return true;
    }
    var fields = document.querySelectorAll('input, textarea, select');
    for (var i = 0; i < fields.length; i += 1) {
      var field = fields[i];
      var type = (field.type || '').toLowerCase();
      if (type === 'hidden' || type === 'submit' || type === 'button' || type === 'file') {
        continue;
      }
      if (type === 'checkbox' || type === 'radio') {
        if (field.checked !== field.defaultChecked) {
          return true;
        }
        continue;
      }
      if (field.tagName === 'SELECT') {
        continue;
      }
      if (field.value !== field.defaultValue) {
        return true;
      }
    }
    return false;
  }

  function showNotice() {
    if (notice) {
      notice.hidden = false;
    }
  }

  function refresh() {
    if (reloading) {
      return;
    }
    if (hasDraft()) {
      showNotice();
      return;
    }
    reloading = true;
    window.location.reload();
  }

  function schedule() {
    window.setTimeout(poll, NOTICE_INTERVAL);
  }

  function poll() {
    window
      .fetch('/events', { cache: 'no-store' })
      .then(function (response) {
        return response.ok ? response.json() : null;
      })
      .then(function (data) {
        if (!data) {
          return;
        }
        if (revision === null) {
          revision = data.rev;
          return;
        }
        if (data.rev !== revision) {
          revision = data.rev;
          refresh();
        }
      })
      .catch(function () {
        // Сервер перезапускается — попробуем на следующем шаге.
      })
      .then(schedule);
  }

  if (reload) {
    reload.addEventListener('click', function () {
      window.location.reload();
    });
  }

  poll();
})();

// Живое обновление страницы: сервер шлёт `change` на каждое изменение базы.
// Если человек что-то печатает, страница не перезагружается — показываем полоску
// с кнопкой, чтобы не потерять введённое.
(function () {
  if (!window.EventSource) {
    return;
  }

  var notice = document.getElementById('live-notice');
  var reload = document.getElementById('live-reload');
  var reloading = false;

  function hasDraft() {
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

  if (reload) {
    reload.addEventListener('click', function () {
      window.location.reload();
    });
  }

  var source = new EventSource('/events');
  source.addEventListener('change', function () {
    if (reloading) {
      return;
    }
    if (hasDraft()) {
      showNotice();
      return;
    }
    reloading = true;
    window.location.reload();
  });
})();

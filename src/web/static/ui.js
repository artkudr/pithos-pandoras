// Раскрытие формы создания в строке таблицы.
// Кнопка «+» в шапке переключает строку `create-row`, форма открывается на месте,
// без перехода на другую страницу.
(function () {
  function setOpen(button, row, open) {
    row.hidden = !open;
    button.setAttribute('aria-expanded', String(open));
    if (!open) {
      return;
    }
    var field = row.querySelector('input, textarea, select');
    if (field) {
      field.focus();
    }
  }

  document.addEventListener('click', function (event) {
    var button = event.target.closest('[data-toggle]');
    if (!button) {
      return;
    }
    var row = document.getElementById(button.getAttribute('data-toggle'));
    if (!row) {
      return;
    }
    event.preventDefault();
    setOpen(button, row, row.hidden);
  });

  document.addEventListener('keydown', function (event) {
    if (event.key !== 'Escape') {
      return;
    }
    var row = document.querySelector('.create-row:not([hidden])');
    if (!row) {
      return;
    }
    var buttons = document.querySelectorAll('[data-toggle="' + row.id + '"]');
    Array.prototype.forEach.call(buttons, function (button) {
      button.setAttribute('aria-expanded', 'false');
    });
    row.hidden = true;
  });
})();

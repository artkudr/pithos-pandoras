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
  function addRoleRow(button) {
    var form = button.closest('form');
    if (!form) {
      return;
    }
    var editor = form.querySelector('#roles-editor tbody');
    var template = form.querySelector('#role-row-template');
    if (!editor || !template) {
      return;
    }
    var row = template.content.firstElementChild.cloneNode(true);
    var index = editor.querySelectorAll('tr').length + 1;
    Array.prototype.forEach.call(row.querySelectorAll('input[type="radio"]'), function (input) {
      input.name = 'role_color_' + index;
    });
    editor.appendChild(row);
    var field = row.querySelector('input[name="role_name"]');
    if (field) {
      field.focus();
    }
  }

  function syncRoles(form) {
    var editor = form.querySelector('#roles-editor tbody');
    var field = form.querySelector('#roles-json');
    if (!editor || !field) {
      return;
    }
    var roles = [];
    Array.prototype.forEach.call(editor.querySelectorAll('tr'), function (row) {
      var name = row.querySelector('input[name="role_name"]');
      if (!name || !name.value.trim()) {
        return;
      }
      var color = row.querySelector('input[name^="role_color"]:checked');
      roles.push({ name: name.value.trim(), color: color ? color.value : null });
    });
    field.value = JSON.stringify(roles);
  }

  document.addEventListener('click', function (event) {
    var add = event.target.closest('[data-add-role]');
    if (add) {
      event.preventDefault();
      addRoleRow(add);
    }
  });

  document.addEventListener('submit', function (event) {
    syncRoles(event.target);
  });
})();

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
  // Таблица ролей: строки, шестерёнка, правка имени и цвета, удаление.
  function initRolesEditor(table) {
    var form = table.closest('form');
    var hidden = form && form.querySelector('#roles-json');
    var template = document.getElementById('role-edit-template');
    var tbody = table.querySelector('tbody');
    if (!form || !hidden || !template || !tbody) {
      return;
    }

    var roles = [];
    try {
      roles = JSON.parse(hidden.value || '[]');
    } catch (error) {
      roles = [];
    }
    var editing = -1;

    function save() {
      hidden.value = JSON.stringify(roles);
    }

    function editorRow(index) {
      var role = roles[index];
      var row = template.content.firstElementChild.cloneNode(true);
      var name = row.querySelector('[data-role-name]');
      name.value = role.name;
      var color = row.querySelector('.color-field');
      var colorValue = color.querySelector('input[type="hidden"]');
      colorValue.value = role.color || '#5c7cfa';
      initColorField(color);

      function commit() {
        var value = name.value.trim();
        if (!value) {
          name.focus();
          return;
        }
        roles[index] = { name: value, color: colorValue.value };
        editing = -1;
        render();
      }

      name.addEventListener('keydown', function (event) {
        if (event.key === 'Enter') {
          event.preventDefault();
          commit();
        } else if (event.key === 'Escape') {
          editing = -1;
          render();
        }
      });
      row.querySelector('[data-role-save]').addEventListener('click', commit);
      row.querySelector('[data-role-cancel]').addEventListener('click', function () {
        if (!role.name) {
          roles.splice(index, 1);
        }
        editing = -1;
        render();
      });
      row.querySelector('[data-role-delete]').addEventListener('click', function () {
        var label = role.name || 'без названия';
        if (!window.confirm('Удалить роль «' + label + '»?')) {
          return;
        }
        roles.splice(index, 1);
        editing = -1;
        render();
      });
      return row;
    }

    function render() {
      tbody.textContent = '';
      roles.forEach(function (role, index) {
        var row = document.createElement('tr');
        var act = document.createElement('td');
        act.className = 'act-col';
        var gear = document.createElement('button');
        gear.type = 'button';
        gear.className = 'row-edit';
        gear.dataset.roleEdit = String(index);
        gear.title = 'Изменить роль';
        gear.setAttribute('aria-label', 'Изменить роль ' + (role.name || 'без названия'));
        gear.innerHTML = '<svg class="icon"><use href="#icon-gear"></use></svg>';
        act.appendChild(gear);

        var nameCell = document.createElement('td');
        var label = document.createElement('span');
        label.textContent = role.name || '—';
        if (role.name && role.color) {
          label.style.color = role.color;
        }
        nameCell.appendChild(label);

        row.appendChild(act);
        row.appendChild(nameCell);
        tbody.appendChild(row);
        if (index === editing) {
          tbody.appendChild(editorRow(index));
        }
      });
      save();
    }

    table.querySelector('[data-add-role]').addEventListener('click', function () {
      roles.push({ name: '', color: '#5c7cfa' });
      editing = roles.length - 1;
      render();
      var input = tbody.querySelector('[data-role-name]');
      if (input) {
        input.focus();
      }
    });

    table.addEventListener('click', function (event) {
      var gear = event.target.closest('[data-role-edit]');
      if (!gear) {
        return;
      }
      editing = Number(gear.dataset.roleEdit);
      render();
      var input = tbody.querySelector('[data-role-name]');
      if (input) {
        input.focus();
      }
    });

    render();
  }

  Array.prototype.forEach.call(document.querySelectorAll('#roles-editor'), initRolesEditor);

  document.addEventListener('submit', function (event) {
    var submitter = event.submitter;
    if (submitter && submitter.dataset.confirm && !window.confirm(submitter.dataset.confirm)) {
      event.preventDefault();
    }
  });

  // Веб-пикер цвета: поле насыщенности и яркости, ползунок оттенка, код цвета.
  function clamp01(value) {
    return Math.min(1, Math.max(0, value));
  }

  function toHex(value) {
    var hex = String(value).trim().replace('#', '');
    if (hex.length === 3) {
      hex = hex[0] + hex[0] + hex[1] + hex[1] + hex[2] + hex[2];
    }
    if (!/^[0-9a-f]{6}$/i.test(hex)) {
      return null;
    }
    return '#' + hex.toLowerCase();
  }

  function hexToHsv(hex) {
    var value = hex.slice(1);
    var r = parseInt(value.slice(0, 2), 16) / 255;
    var g = parseInt(value.slice(2, 4), 16) / 255;
    var b = parseInt(value.slice(4, 6), 16) / 255;
    var max = Math.max(r, g, b);
    var min = Math.min(r, g, b);
    var d = max - min;
    var h = 0;
    if (d !== 0) {
      if (max === r) {
        h = ((g - b) / d) % 6;
      } else if (max === g) {
        h = (b - r) / d + 2;
      } else {
        h = (r - g) / d + 4;
      }
      h *= 60;
      if (h < 0) {
        h += 360;
      }
    }
    return { h: h, s: max === 0 ? 0 : d / max, v: max };
  }

  function hsvToHex(h, s, v) {
    var c = v * s;
    var x = c * (1 - Math.abs(((h / 60) % 2) - 1));
    var m = v - c;
    var rgb;
    if (h < 60) {
      rgb = [c, x, 0];
    } else if (h < 120) {
      rgb = [x, c, 0];
    } else if (h < 180) {
      rgb = [0, c, x];
    } else if (h < 240) {
      rgb = [0, x, c];
    } else if (h < 300) {
      rgb = [x, 0, c];
    } else {
      rgb = [c, 0, x];
    }
    return '#' + rgb.map(function (channel) {
      var part = Math.round((channel + m) * 255).toString(16);
      return part.length === 1 ? '0' + part : part;
    }).join('');
  }

  function initColorField(field) {
    if (field.dataset.colorReady === '1') {
      return;
    }
    var button = field.querySelector('.color-button');
    var popover = field.querySelector('.color-popover');
    var area = field.querySelector('.color-area');
    var handle = field.querySelector('.color-handle');
    var hue = field.querySelector('.color-hue');
    var preview = field.querySelector('.color-preview');
    var hexInput = field.querySelector('.color-hex');
    var dot = field.querySelector('.color-dot');
    var hidden = field.querySelector('input[type="hidden"]');
    if (!button || !popover || !area || !hidden) {
      return;
    }
    field.dataset.colorReady = '1';

    var state = hexToHsv(toHex(hidden.value) || '#5c7cfa');

    function render() {
      var hex = hsvToHex(state.h, state.s, state.v);
      var hueColor = 'hsl(' + Math.round(state.h) + ', 100%, 50%)';
      hidden.value = hex;
      area.style.setProperty('--hue', hueColor);
      hue.style.setProperty('--hue', hueColor);
      hue.value = String(Math.round(state.h));
      handle.style.left = state.s * 100 + '%';
      handle.style.top = (1 - state.v) * 100 + '%';
      dot.style.setProperty('--current', hex);
      preview.style.setProperty('--current', hex);
      if (document.activeElement !== hexInput) {
        hexInput.value = hex;
      }
    }

    function close() {
      popover.hidden = true;
      button.setAttribute('aria-expanded', 'false');
    }

    function open() {
      popover.hidden = false;
      button.setAttribute('aria-expanded', 'true');
      var rect = button.getBoundingClientRect();
      var width = popover.offsetWidth || 220;
      var height = popover.offsetHeight || 220;
      var left = Math.min(Math.max(8, rect.left), window.innerWidth - width - 8);
      var top = rect.bottom + 6;
      if (top + height > window.innerHeight - 8) {
        top = Math.max(8, rect.top - height - 6);
      }
      popover.style.left = left + 'px';
      popover.style.top = top + 'px';
      area.focus();
    }

    button.addEventListener('click', function () {
      if (popover.hidden) {
        open();
      } else {
        close();
      }
    });

    document.addEventListener('pointerdown', function (event) {
      if (!popover.hidden && !field.contains(event.target)) {
        close();
      }
    });

    document.addEventListener('keydown', function (event) {
      if (event.key === 'Escape' && !popover.hidden) {
        close();
        button.focus();
      }
    });

    window.addEventListener('resize', close);
    document.addEventListener('scroll', close, true);

    function pick(event) {
      var rect = area.getBoundingClientRect();
      state.s = clamp01((event.clientX - rect.left) / rect.width);
      state.v = 1 - clamp01((event.clientY - rect.top) / rect.height);
      render();
    }

    area.addEventListener('pointerdown', function (event) {
      event.preventDefault();
      area.setPointerCapture(event.pointerId);
      pick(event);
    });

    area.addEventListener('pointermove', function (event) {
      if (area.hasPointerCapture(event.pointerId)) {
        pick(event);
      }
    });

    area.addEventListener('pointerup', function (event) {
      if (area.hasPointerCapture(event.pointerId)) {
        area.releasePointerCapture(event.pointerId);
      }
    });

    area.addEventListener('keydown', function (event) {
      var step = 0.05;
      if (event.key === 'ArrowLeft') {
        state.s = clamp01(state.s - step);
      } else if (event.key === 'ArrowRight') {
        state.s = clamp01(state.s + step);
      } else if (event.key === 'ArrowDown') {
        state.v = clamp01(state.v - step);
      } else if (event.key === 'ArrowUp') {
        state.v = clamp01(state.v + step);
      } else {
        return;
      }
      event.preventDefault();
      render();
    });

    hue.addEventListener('input', function () {
      state.h = Number(hue.value);
      render();
    });

    hexInput.addEventListener('input', function () {
      var value = toHex(hexInput.value);
      if (!value) {
        return;
      }
      state = hexToHsv(value);
      render();
    });

    render();
  }

  Array.prototype.forEach.call(document.querySelectorAll('.color-field'), initColorField);
})();

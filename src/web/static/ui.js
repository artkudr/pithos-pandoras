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
    var color = row.querySelector('.color-field');
    if (color) {
      var hidden = color.querySelector('input[type="hidden"]');
      if (hidden) {
        hidden.name = 'role_color_' + index;
      }
      initColorField(color);
    }
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
      var color = row.querySelector('input[name^="role_color"]');
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

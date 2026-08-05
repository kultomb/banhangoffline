/**
 * license-gate.js — HangHoa POS License Gate
 *
 * Loaded BEFORE app.js. Checks the offline license via Tauri IPC and either:
 *   • hides the overlay if the license is valid → app runs normally
 *   • shows the activation form if unlicensed / trial-expired / expired
 *
 * All Tauri commands used:
 *   cmd_check_license()  → LicenseStatus JSON
 *   cmd_activate_license(key)  → Result<LicenseStatus, String>
 *   cmd_start_trial()    → Result<LicenseStatus, String>
 */
(function () {
    'use strict';

    // --- Tauri IPC helper (same pattern as app.js) ----------------------------------------
    function tauriInvoke(cmd, args) {
        try {
            var internals =
                window.__TAURI_INTERNALS__ ||
                (window.__TAURI__ && window.__TAURI__.core);
            if (internals && typeof internals.invoke === 'function') {
                return Promise.resolve(internals.invoke(cmd, args || {}));
            }
        } catch (e) {
            return Promise.reject(e);
        }
        return Promise.resolve(null);
    }

    function isTauri() {
        return (
            typeof window.__TAURI_INTERNALS__ !== 'undefined' ||
            typeof window.__TAURI__ !== 'undefined'
        );
    }

    // Resolve after ms milliseconds — used for timeout races
    function delay(ms) {
        return new Promise(function (resolve) { setTimeout(resolve, ms); });
    }

    // --- DOM refs -------------------------------------------------------------------------
    var gate = document.getElementById('license-gate');
    var lgLoading = document.getElementById('lg-loading');
    var lgCard = document.getElementById('lg-card');
    var lgContent = document.getElementById('lg-content');
    var lgSubtitle = document.getElementById('lg-subtitle');

    // --- Helpers -------------------------------------------------------------------------
    function showCard(subtitleText) {
        lgLoading.style.display = 'none';
        lgCard.style.display = 'block';
        if (subtitleText) lgSubtitle.textContent = subtitleText;
    }

    function hideGate() {
        gate.style.transition = 'opacity .35s';
        gate.style.opacity = '0';
        gate.style.pointerEvents = 'none';
        setTimeout(function () { gate.style.display = 'none'; }, 370);
    }

    function copyText(text) {
        if (navigator.clipboard) {
            navigator.clipboard.writeText(text).catch(function () {
                fallbackCopy(text);
            });
        } else {
            fallbackCopy(text);
        }
    }

    function fallbackCopy(text) {
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.cssText = 'position:fixed;opacity:0;';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        document.body.removeChild(ta);
    }

    function escapeHtml(s) {
        return String(s)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function showMsg(el, msg, type) {
        el.className = 'lg-msg ' + (type === 'error' ? 'is-error' : 'is-success');
        el.textContent = msg;
    }

    function setError(el, msg) {
        if (!el) return;
        el.style.display = 'block';
        el.style.color = '#991b1b';
        el.style.background = '#fee2e2';
        el.style.border = '1px solid #fca5a5';
        el.textContent = msg;
    }

    function setSuccess(el, msg) {
        if (!el) return;
        el.style.display = 'block';
        el.style.color = '#166534';
        el.style.background = '#dcfce7';
        el.style.border = '1px solid #86efac';
        el.textContent = msg;
    }

    function showSuccessModal(text, onOk) {
        var modal = document.getElementById('lg-success-modal');
        var textEl = document.getElementById('lg-success-text');
        var okBtn = document.getElementById('lg-success-ok');
        if (!modal || !textEl || !okBtn) {
            if (onOk) onOk();
            return;
        }
        textEl.textContent = text || 'Cảm ơn bạn đã đăng ký bản quyền!';
        modal.classList.add('is-open');
        okBtn.onclick = function () {
            modal.classList.remove('is-open');
            if (onOk) onOk();
        };
    }

    // --- Edition banner (shown in app bar after activation) ----------------------------
    function injectEditionBadge(status) {
        window.__licenseStatus = status;
        window.__licenseEdition = status.edition || 'FREE';
        window.__licenseActive = (status.status === 'Active' || status.status === 'Trial');

        // Wait for DOM to be ready, then inject a small badge in the top bar
        function doInject() {
            var slot = document.getElementById('next-account-slot');
            if (!slot) return;
            var existing = document.getElementById('hhpos-license-badge');
            if (existing) existing.remove();

            var badgeColors = {
                PRO: { bg: '#7c3aed', text: '#fff' },
                LIFETIME: { bg: '#b45309', text: '#fff' },
                BASIC: { bg: '#0369a1', text: '#fff' },
                FREE: { bg: '#6b7280', text: '#fff' },
                TRIAL: { bg: '#059669', text: '#fff' },
            };

            var editionKey = status.status === 'Trial' ? 'TRIAL' : (status.edition || 'FREE');
            var col = badgeColors[editionKey] || badgeColors.FREE;

            var badgeLabel = status.status === 'Trial'
                ? 'TRIAL (' + status.days_left + ' ngày)'
                : (status.edition || 'FREE');

            var badge = document.createElement('div');
            badge.id = 'hhpos-license-badge';
            badge.title = 'Trạng thái bản quyền — click để xem chi tiết';
            badge.style.cssText = [
                'display:flex;align-items:center;gap:6px;',
                'background:' + col.bg + ';color:' + col.text + ';',
                'border-radius:8px;padding:5px 12px;font-size:12px;font-weight:700;',
                'cursor:pointer;white-space:nowrap;user-select:none;',
                'letter-spacing:.5px;',
            ].join('');
            badge.innerHTML = '🔒 ' + badgeLabel;
            badge.addEventListener('click', function () {
                showLicenseInfoModal(status);
            });
            slot.appendChild(badge);
        }

        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', doInject);
        } else {
            doInject();
        }
    }

    // --- License info modal (accessible from badge click) -----------------------------
    function showLicenseInfoModal(status) {
        var existing = document.getElementById('lg-info-modal');
        if (existing) { existing.remove(); return; }

        var lines = [];
        if (status.status === 'Active') {
            lines.push('Trạng thái : Đã kích hoạt ✓');
            lines.push('Edition    : ' + status.edition);
            lines.push('Hạn dùng   : ' + (status.expiry_str || 'Vĩnh viễn'));
            if (status.days_left !== null && status.days_left !== undefined) {
                lines.push('Còn lại    : ' + status.days_left + ' ngày');
            }
        } else if (status.status === 'Trial') {
            lines.push('Trạng thái : Dùng thử');
            lines.push('Còn lại    : ' + status.days_left + ' ngày');
        }
        lines.push('Mã máy     : ' + (status.machine_id || '—'));
        var modal = document.createElement('div');
        modal.id = 'lg-info-modal';
        modal.style.cssText = [
            'position:fixed;top:60px;right:16px;z-index:2147483640;',
            'background:#fff;border-radius:14px;padding:20px 24px;',
            'box-shadow:0 12px 40px rgba(0,0,0,.25);',
            'font-family:Segoe UI,system-ui,sans-serif;font-size:13px;',
            'color:#1e293b;min-width:280px;',
        ].join('');
        modal.innerHTML = '<div style="font-weight:700;font-size:15px;margin-bottom:12px;">🔒 Thông tin bản quyền</div>'
            + '<pre style="margin:0;font-size:12px;font-family:monospace;line-height:1.7;white-space:pre-wrap;">'
            + lines.join('\n') + '</pre>'
            + '<div style="margin-top:14px;display:flex;gap:8px;flex-wrap:wrap;">'
            + '<button onclick="document.getElementById(\'lg-info-modal\').remove();window.lgShowActivation&&window.lgShowActivation();" '
            + 'style="padding:7px 14px;background:#7c3aed;color:#fff;border:none;border-radius:8px;font-weight:600;cursor:pointer;font-size:12px;">'
            + 'Nhập key mới</button>'
            + '<button onclick="document.getElementById(\'lg-info-modal\').remove();" '
            + 'style="padding:7px 14px;background:#e2e8f0;color:#334155;border:none;border-radius:8px;font-weight:600;cursor:pointer;font-size:12px;">'
            + 'Đóng</button>'
            + '</div>';

        document.body.appendChild(modal);

        // Close on outside click
        setTimeout(function () {
            document.addEventListener('click', function handler(e) {
                if (!modal.contains(e.target)) {
                    modal.remove();
                    document.removeEventListener('click', handler);
                }
            });
        }, 100);
    }

    // --- Registration form (Hardware ID + Name + Key — giống ảnh) ------------------------
    function wireRegistrationForm(machineId, opts) {
        opts = opts || {};
        var allowTrial = !!opts.allowTrial;
        var activateLabel = opts.activateLabel || 'Kích hoạt';

        document.getElementById('lg-copy-btn').addEventListener('click', function () {
            copyText(machineId);
            this.title = 'Đã sao chép!';
            var btn = this;
            setTimeout(function () { btn.title = 'Sao chép mã máy'; }, 2000);
        });

        var msgEl = document.getElementById('lg-msg');
        var activateBtn = document.getElementById('lg-activate-btn');
        var keyInput = document.getElementById('lg-key-input');

        function resetActivateBtn() {
            activateBtn.disabled = false;
            activateBtn.textContent = activateLabel;
        }

        activateBtn.addEventListener('click', function () {
            var key = keyInput.value.trim();
            if (!key) {
                showMsg(msgEl, 'Vui lòng dán License Key vào ô Key.', 'error');
                return;
            }
            activateBtn.disabled = true;
            activateBtn.textContent = 'Đang xác thực…';
            msgEl.className = 'lg-msg';

            tauriInvoke('cmd_activate_license', { key: key })
                .then(function (status) {
                    if (!status) {
                        showMsg(msgEl, 'Lỗi kết nối. Vui lòng thử lại.', 'error');
                        resetActivateBtn();
                        return;
                    }
                    showSuccessModal('Cảm ơn bạn đã đăng ký bản quyền!', function () {
                        injectEditionBadge(status);
                        hideGate();
                    });
                })
                .catch(function (err) {
                    showMsg(msgEl, String(err || 'Kích hoạt thất bại.'), 'error');
                    resetActivateBtn();
                });
        });

        if (allowTrial) {
            var trialBtn = document.getElementById('lg-trial-btn');
            if (trialBtn) {
                trialBtn.addEventListener('click', function () {
                    trialBtn.disabled = true;
                    trialBtn.textContent = 'Đang kích hoạt…';
                    msgEl.className = 'lg-msg';

                    tauriInvoke('cmd_start_trial', {})
                        .then(function (status) {
                            if (!status) {
                                showMsg(msgEl, 'Lỗi kết nối.', 'error');
                                trialBtn.disabled = false;
                                trialBtn.textContent = 'Dùng thử 7 ngày';
                                return;
                            }
                            showSuccessModal(
                                'Bắt đầu dùng thử ' + (status.days_left || 7) + ' ngày!',
                                function () {
                                    injectEditionBadge(status);
                                    hideGate();
                                }
                            );
                        })
                        .catch(function (err) {
                            showMsg(msgEl, String(err || 'Không thể kích hoạt dùng thử.'), 'error');
                            trialBtn.disabled = false;
                            trialBtn.textContent = 'Dùng thử 7 ngày';
                        });
                });
            }
        }

        window.lgShowActivation = function () {
            gate.style.display = 'flex';
            gate.style.opacity = '1';
            gate.style.pointerEvents = '';
        };
    }

    function renderRegistrationForm(machineId, opts) {
        opts = opts || {};
        var mid = escapeHtml(machineId);
        var banner = opts.bannerHtml || '';
        var trialBtnHtml = opts.allowTrial
            ? '<button type="button" id="lg-trial-btn" class="lg-btn lg-btn-secondary">Dùng thử 7 ngày</button>'
            : '';
        var activateLabel = escapeHtml(opts.activateLabel || 'Kích hoạt');

        lgContent.innerHTML = [
            banner,
            '<fieldset class="lg-group">',
            '  <legend>Hardware ID</legend>',
            '  <div class="lg-hwid-row">',
            '    <input type="text" id="lg-mid" class="lg-hwid-input" readonly value="' + mid + '">',
            '    <button type="button" id="lg-copy-btn" class="lg-icon-btn" title="Sao chép mã máy">📋</button>',
            '  </div>',
            '  <p class="lg-hint">Gửi mã này cho nhà cung cấp để nhận License Key, sau đó dán vào ô Key bên dưới.</p>',
            '</fieldset>',
            '<fieldset class="lg-group">',
            '  <legend>Registration Information</legend>',
            '  <div class="lg-field">',
            '    <label for="lg-name">Name</label>',
            '    <input type="text" id="lg-name" class="lg-input" placeholder="Tên cửa hàng / khách hàng (tuỳ chọn)" autocomplete="name">',
            '  </div>',
            '  <div class="lg-field">',
            '    <label for="lg-key-input">Key</label>',
            '    <textarea id="lg-key-input" class="lg-textarea" rows="7" placeholder="Dán license key tại đây…"></textarea>',
            '  </div>',
            '</fieldset>',
            '<div id="lg-msg" class="lg-msg"></div>',
            '<div class="lg-actions">',
            trialBtnHtml,
            '  <button type="button" id="lg-activate-btn" class="lg-btn lg-btn-primary">' + activateLabel + '</button>',
            '</div>',
            '<p class="lg-footer-note">Liên hệ mua bản quyền: <strong>hangho.com</strong></p>',
        ].join('');

        wireRegistrationForm(machineId, opts);
    }

    function renderActivationForm(machineId, allowTrial) {
        renderRegistrationForm(machineId, {
            allowTrial: allowTrial,
            activateLabel: 'Kích hoạt',
        });
    }


    // --- Render: expired / trial-expired state -----------------------------------------
    function renderExpiredState(machineId, isTrialExpired, edition) {
        var icon = isTrialExpired ? '⌛' : '⚠️';
        var title = isTrialExpired ? 'Hết thời gian dùng thử' : 'Bản quyền đã hết hạn';
        var detail = isTrialExpired
            ? '7 ngày dùng thử đã kết thúc.'
            : 'Bản quyền <strong>' + (edition || '') + '</strong> của bạn đã hết hạn.';

        lgContent.innerHTML = [
            '<div style="text-align:center;margin-bottom:20px;">',
            '  <div style="font-size:48px;margin-bottom:8px;">' + icon + '</div>',
            '  <h2 style="margin:0 0 8px;font-size:18px;color:#dc2626;">' + title + '</h2>',
            '  <p style="margin:0;font-size:14px;color:#64748b;">' + detail + '</p>',
            '</div>',
            '<div style="background:#f1f5f9;border-radius:12px;padding:12px 16px;margin-bottom:20px;font-size:12px;color:#475569;">',
            '  <strong>Mã máy:</strong> <code style="font-family:monospace;letter-spacing:1px;">' + machineId + '</code>',
            '</div>',
            '<div style="margin-bottom:12px;">',
            '  <label style="font-size:13px;font-weight:600;color:#334155;display:block;margin-bottom:6px;">Nhập License Key mới để tiếp tục</label>',
            '  <textarea id="lg-key-input" rows="3" placeholder="Dán license key vào đây…" style="',
            '    width:100%;box-sizing:border-box;border:1.5px solid #cbd5e1;border-radius:10px;',
            '    padding:10px 12px;font-size:12px;font-family:monospace;resize:vertical;color:#1e293b;outline:none;',
            '  "></textarea>',
            '</div>',
            '<div id="lg-msg" style="font-size:13px;margin-bottom:12px;display:none;padding:10px 14px;border-radius:9px;"></div>',
            '<button id="lg-activate-btn" style="',
            '  width:100%;padding:13px;background:linear-gradient(135deg,#7c3aed,#8b5cf6);',
            '  color:#fff;border:none;border-radius:11px;font-size:15px;font-weight:700;cursor:pointer;',
            '">🔒 Kích hoạt bản quyền mới</button>',
            '<div style="margin-top:14px;text-align:center;font-size:12px;color:#94a3b8;">Liên hệ: <strong style="color:#475569;">hangho.com</strong></div>',
        ].join('');

        var msgEl = document.getElementById('lg-msg');
        var activateBtn = document.getElementById('lg-activate-btn');

        activateBtn.addEventListener('click', function () {
            var key = document.getElementById('lg-key-input').value.trim();
            if (!key) {
                msgEl.style.background = '#fee2e2';
                setError(msgEl, 'Vui lòng dán license key vào ô trên.');
                return;
            }
            activateBtn.disabled = true;
            activateBtn.textContent = '⏳ Đang xác thực…';
            msgEl.style.display = 'none';

            tauriInvoke('cmd_activate_license', { key: key })
                .then(function (status) {
                    if (!status) { throw new Error('Lỗi kết nối.'); }
                    msgEl.style.background = '#dcfce7';
                    setSuccess(msgEl, '✔ Kích hoạt thành công!');
                    injectEditionBadge(status);
                    setTimeout(hideGate, 1200);
                })
                .catch(function (err) {
                    msgEl.style.background = '#fee2e2';
                    setError(msgEl, '❌ ' + (err || 'Kích hoạt thất bại.'));
                    activateBtn.disabled = false;
                    activateBtn.textContent = '🔒 Kích hoạt bản quyền mới';
                });
        });
    }

    // --- Render: time manipulation warning --------------------------------------------------
    function renderTimeWarning(machineId) {
        lgContent.innerHTML = [
            '<div style="text-align:center;margin-bottom:20px;">',
            '  <div style="font-size:48px;margin-bottom:8px;">⚠️</div>',
            '  <h2 style="margin:0 0 8px;font-size:18px;color:#d97706;">Cảnh báo đồng hồ hệ thống</h2>',
            '  <p style="margin:0;font-size:14px;color:#64748b;line-height:1.6;">',
            '    Đồng hồ máy tính của bạn bị đặt về thời gian cũ hơn lần chạy trước.<br>',
            '    Vui lòng chỉnh lại thời gian hệ thống chính xác rồi khởi động lại phần mềm.',
            '  </p>',
            '</div>',
            '<div style="background:#fef3c7;border-radius:12px;padding:14px 16px;border:1px solid #fcd34d;font-size:13px;color:#92400e;">',
            '  <strong>Cách khắc phục:</strong> Vào Settings → Time &amp; Language → chọn "Set time automatically".',
            '</div>',
            '<div style="margin-top:18px;text-align:center;font-size:12px;color:#94a3b8;">Mã máy: <code>' + machineId + '</code></div>',
        ].join('');
    }

    // --- Main: run license check --------------------------------------------------------
    var _checkDone = false; // prevent double-execution

    function handleStatus(status) {
        if (_checkDone) return;
        _checkDone = true;

        if (!status) {
            // IPC returned null / command not found — allow app to run
            hideGate();
            return;
        }

        var mid = status.machine_id || '????-????-????';

        switch (status.status) {
            case 'Active':
                injectEditionBadge(status);
                hideGate();
                break;

            case 'Trial':
                injectEditionBadge(status);
                hideGate();
                break;

            case 'Unlicensed':
                showCard('Chưa kích hoạt bản quyền');
                renderActivationForm(mid, true);
                break;

            case 'Expired':
                showCard('Bản quyền đã hết hạn');
                renderExpiredState(mid, false, status.edition);
                break;

            case 'TrialExpired':
                showCard('Hết thời gian dùng thử');
                renderExpiredState(mid, true, null);
                break;

            case 'TimeManipulation':
                showCard('Cảnh báo bảo mật');
                renderTimeWarning(mid);
                break;

            default:
                hideGate();
        }
    }

    function runLicenseCheck() {
        if (!isTauri()) {
            // Running in a browser (dev mode) — skip gate entirely
            window.__licenseEdition = 'PRO';
            window.__licenseActive = true;
            gate.style.display = 'none';
            return;
        }

        // Safety timeout: if IPC hangs for any reason, don't block the app forever.
        // After 6 seconds the gate hides so users are never locked out by an infra bug.
        delay(6000).then(function () {
            if (!_checkDone) {
                console.warn('[LicenseGate] timeout — IPC did not respond in 6s, allowing app');
                handleStatus(null); // treats as "no status" → hideGate
            }
        });

        tauriInvoke('cmd_check_license', {})
            .then(function (status) { handleStatus(status); })
            .catch(function (err) {
                console.warn('[LicenseGate] check_license error:', err);
                if (!_checkDone) {
                    _checkDone = true;
                    // If command not found (old binary) or any error → hide gate
                    hideGate();
                }
            });
    }

    // Run immediately (DOM is already parsed since script is at end of body)
    runLicenseCheck();

})();

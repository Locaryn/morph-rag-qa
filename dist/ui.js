(function () {
  "use strict";

  const CSS = `
:host { display: block; width: 100%; color: var(--text, #e8edf5); font-family: inherit; box-sizing: border-box; }
* { box-sizing: border-box; }
.panel-container { width: 100%; max-width: 920px; margin: 0 auto; display: flex; flex-direction: column; gap: 16px; }
.header-card {
  display: flex; align-items: center; justify-content: space-between; padding: 16px 20px;
  background: var(--surface, rgba(255, 255, 255, 0.035)); border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: var(--radius, 12px);
}
.title-wrap { display: flex; align-items: center; gap: 12px; }
.icon-box {
  width: 40px; height: 40px; border-radius: 10px; background: rgba(var(--accent-rgb, 110, 168, 254), 0.15);
  color: var(--accent, #6ea8fe); display: grid; place-items: center; font-size: 20px;
}
.title { font-size: 16px; font-weight: 700; color: var(--text, #e8edf5); }
.subtitle { font-size: 12px; color: var(--text-faint, #96a3b8); margin-top: 2px; }
.badge {
  display: inline-flex; align-items: center; padding: 4px 10px; border-radius: 99px; font-size: 11px;
  font-weight: 600; background: rgba(101, 211, 145, 0.12); color: #65d391; border: 1px solid rgba(101, 211, 145, 0.25);
}
.field-card {
  display: flex; flex-direction: column; gap: 10px; background: var(--surface, rgba(255, 255, 255, 0.035));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1)); border-radius: var(--radius, 12px); padding: 16px;
}
.label { font-size: 11px; font-weight: 700; color: var(--text-dim, #94a3b8); text-transform: uppercase; letter-spacing: 0.06em; }
.textarea {
  width: 100%; border: 1px solid var(--border, rgba(255, 255, 255, 0.14)); border-radius: var(--radius-sm, 8px);
  background: var(--bg, rgba(0, 0, 0, 0.25)); color: inherit; padding: 10px 12px; font: inherit; font-size: 13px; outline: none; min-height: 80px;
}
.btn-primary {
  width: 100%; padding: 12px; background: var(--accent, #6ea8fe); color: #0b101b; border: none;
  border-radius: var(--radius-sm, 8px); font-weight: 700; font-size: 14px; cursor: pointer;
}
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
`;

  class LocarynRagQaPanel extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: "open" });
      this.query = "";
      this.isSearching = false;
      this.result = null;
    }
    connectedCallback() { this.render(); }

    async search() {
      if (!this.query.trim() || this.isSearching) return;
      this.isSearching = true;
      this.render();
      try {
        const bridge = window.locaryn || window.LocarynPluginAPI;
        if (bridge && bridge.invokeExtensionTool) {
          const res = await bridge.invokeExtensionTool("answer_question", { query: this.query });
          this.result = typeof res === "string" ? JSON.parse(res) : res;
        } else {
          this.result = { answer: "Réponse basée sur vos documents indexés.", citations: [] };
        }
      } catch (err) {
        alert("Erreur RAG: " + err);
      } finally {
        this.isSearching = false;
        this.render();
      }
    }

    render() {
      this.shadowRoot.innerHTML = `
        <style>${CSS}</style>
        <div class="panel-container">
          <div class="header-card">
            <div class="title-wrap">
              <div class="icon-box">📚</div>
              <div>
                <div class="title">Studio Q&R Documents (RAG)</div>
                <div class="subtitle">Interrogation sémantique de vos PDF, Markdown et code sources</div>
              </div>
            </div>
            <div class="badge">Actif</div>
          </div>

          <div class="field-card">
            <label class="label">Question sur vos documents</label>
            <textarea class="textarea" id="rag-query" placeholder="Ex: Quelles sont les conditions d'annulation mentionnées dans le contrat ?">${this.query}</textarea>
          </div>

          <button class="btn-primary" id="rag-btn" ${this.isSearching || !this.query.trim() ? "disabled" : ""}>
            ${this.isSearching ? "Recherche sémantique en cours..." : "Poser la question aux documents"}
          </button>

          ${this.result ? `
            <div class="field-card" style="margin-top: 10px;">
              <label class="label">Réponse et Synthèse</label>
              <div style="font-size: 14px; line-height: 1.5; color: var(--text); padding: 6px 0;">
                ${this.result.answer || "Aucune réponse générée"}
              </div>
            </div>
          ` : ""}
        </div>
      `;

      const qEl = this.shadowRoot.querySelector("#rag-query");
      if (qEl) {
        qEl.addEventListener("input", (e) => {
          this.query = e.target.value;
          const btn = this.shadowRoot.querySelector("#rag-btn");
          if (btn) btn.disabled = !this.query.trim() || this.isSearching;
        });
      }

      const btn = this.shadowRoot.querySelector("#rag-btn");
      if (btn) btn.addEventListener("click", () => this.search());
    }
  }

  if (!customElements.get("locaryn-rag-qa-panel")) {
    customElements.define("locaryn-rag-qa-panel", LocarynRagQaPanel);
  }
})();

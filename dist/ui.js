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
.textarea, .input {
  width: 100%; border: 1px solid var(--border, rgba(255, 255, 255, 0.14)); border-radius: var(--radius-sm, 8px);
  background: var(--bg, rgba(0, 0, 0, 0.25)); color: inherit; padding: 10px 12px; font: inherit; font-size: 13px; outline: none;
}
.textarea { min-height: 80px; }
.row { display: flex; gap: 10px; align-items: flex-end; }
.row .input { flex: 1; }
.btn-primary {
  padding: 12px; background: var(--accent, #6ea8fe); color: #0b101b; border: none;
  border-radius: var(--radius-sm, 8px); font-weight: 700; font-size: 14px; cursor: pointer; white-space: nowrap;
}
.btn-primary.full { width: 100%; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-secondary {
  padding: 10px 14px; background: transparent; color: var(--text-dim, #94a3b8);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.14)); border-radius: var(--radius-sm, 8px);
  font-size: 12px; cursor: pointer;
}
.error-card {
  padding: 14px 16px; background: rgba(240, 100, 100, 0.08); border: 1px solid rgba(240, 100, 100, 0.25);
  border-radius: var(--radius, 12px); color: #f08a8a; font-size: 13px; line-height: 1.5;
}
.note-card {
  padding: 14px 16px; background: var(--surface, rgba(255, 255, 255, 0.035)); border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: var(--radius, 12px); color: var(--text-faint, #96a3b8); font-size: 13px; line-height: 1.5;
}
.citation {
  padding: 12px 14px; background: var(--bg, rgba(0, 0, 0, 0.2)); border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: var(--radius-sm, 8px); font-size: 13px; line-height: 1.5;
}
.citation-head { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-dim, #94a3b8); margin-bottom: 6px; }
.status-line { font-size: 12px; color: var(--text-faint, #96a3b8); }
.sources { display: flex; flex-direction: column; gap: 4px; margin-top: 6px; }
.source-item { font-size: 12px; color: var(--text-dim, #94a3b8); }
`;

  class LocarynRagQaPanel extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: "open" });
      this.query = "";
      this.filePath = "";
      this.isSearching = false;
      this.isIndexing = false;
      this.result = null;
      this.searchError = null;
      this.indexError = null;
      this.indexNotice = null;
      this.status = null;
    }

    connectedCallback() {
      this.render();
      void this.refreshStatus();
    }

    bridge() {
      return window.locaryn || window.LocarynPluginAPI || null;
    }

    async call(tool, args) {
      const bridge = this.bridge();
      if (!bridge || !bridge.invokeExtensionTool) {
        throw new Error("Le pont d'extension n'est pas disponible dans ce contexte.");
      }
      const res = await bridge.invokeExtensionTool(tool, args);
      return typeof res === "string" ? JSON.parse(res) : res;
    }

    async refreshStatus() {
      try {
        this.status = await this.call("rag_status", {});
      } catch (err) {
        this.status = null;
      } finally {
        this.render();
      }
    }

    async indexer() {
      if (!this.filePath.trim() || this.isIndexing) return;
      this.isIndexing = true;
      this.indexError = null;
      this.indexNotice = null;
      this.render();
      try {
        const res = await this.call("index_document", { file_path: this.filePath.trim() });
        this.indexNotice = `${res.chunks_indexed} morceau(x) indexé(s) depuis ${res.source} (${res.total_chunks} au total dans l'index).`;
        this.filePath = "";
        await this.refreshStatus();
      } catch (err) {
        this.indexError = err && err.message ? err.message : String(err);
      } finally {
        this.isIndexing = false;
        this.render();
      }
    }

    async search() {
      if (!this.query.trim() || this.isSearching) return;
      this.isSearching = true;
      this.searchError = null;
      this.result = null;
      this.render();
      try {
        this.result = await this.call("answer_question", { query: this.query });
      } catch (err) {
        this.searchError = err && err.message ? err.message : String(err);
      } finally {
        this.isSearching = false;
        this.render();
      }
    }

    render() {
      const aIndexe = this.status && this.status.chunks > 0;

      this.shadowRoot.innerHTML = `
        <style>${CSS}</style>
        <div class="panel-container">
          <div class="header-card">
            <div class="title-wrap">
              <div class="icon-box">📚</div>
              <div>
                <div class="title">Studio Q&R Documents (RAG)</div>
                <div class="subtitle">Recherche sémantique locale dans vos documents indexés</div>
              </div>
            </div>
            <div class="badge">Actif</div>
          </div>

          <div class="field-card">
            <label class="label">Documents indexés</label>
            <div class="status-line">
              ${
                this.status === null
                  ? "État inconnu — le moteur d'embeddings répond-il ?"
                  : aIndexe
                    ? `${this.status.chunks} morceau(x), modèle « ${this.status.embed_model} »`
                    : "Aucun document indexé pour le moment."
              }
            </div>
            ${
              this.status && this.status.sources.length > 0
                ? `<div class="sources">${this.status.sources.map((s) => `<div class="source-item">${s}</div>`).join("")}</div>`
                : ""
            }

            <div class="row" style="margin-top: 8px;">
              <input class="input" id="rag-path" type="text" placeholder="Chemin du fichier à indexer (.md, .txt, .pdf converti…)" value="${this.filePath}" />
              <button class="btn-primary" id="rag-index-btn" ${this.isIndexing || !this.filePath.trim() ? "disabled" : ""}>
                ${this.isIndexing ? "Indexation…" : "Indexer"}
              </button>
            </div>
            ${this.indexError ? `<div class="error-card">${this.indexError}</div>` : ""}
            ${this.indexNotice ? `<div class="note-card">${this.indexNotice}</div>` : ""}
          </div>

          <div class="field-card">
            <label class="label">Question sur vos documents</label>
            <textarea class="textarea" id="rag-query" placeholder="Ex: Quelles sont les conditions d'annulation mentionnées dans le contrat ?">${this.query}</textarea>
          </div>

          <button class="btn-primary full" id="rag-btn" ${this.isSearching || !this.query.trim() ? "disabled" : ""}>
            ${this.isSearching ? "Recherche sémantique en cours..." : "Poser la question aux documents"}
          </button>

          ${this.searchError ? `<div class="error-card">${this.searchError}</div>` : ""}

          ${
            this.result
              ? `
            <div class="field-card" style="margin-top: 4px;">
              <label class="label">Passages retrouvés</label>
              <div class="note-card">
                ${this.result.note}<br><br>
                Ce morph ne rédige pas de réponse toute faite — il retrouve les passages les
                plus proches de la question, avec leur source, pour que vous jugiez vous-même
                s'ils y répondent.
              </div>
              ${this.result.citations
                .map(
                  (c) => `
                <div class="citation">
                  <div class="citation-head"><span>${c.file_path}</span><span>score ${c.score.toFixed(2)}</span></div>
                  <div>${c.snippet}</div>
                </div>
              `,
                )
                .join("")}
            </div>
          `
              : ""
          }
        </div>
      `;

      const pathEl = this.shadowRoot.querySelector("#rag-path");
      if (pathEl) {
        pathEl.addEventListener("input", (e) => {
          this.filePath = e.target.value;
          const btn = this.shadowRoot.querySelector("#rag-index-btn");
          if (btn) btn.disabled = !this.filePath.trim() || this.isIndexing;
        });
      }

      const indexBtn = this.shadowRoot.querySelector("#rag-index-btn");
      if (indexBtn) indexBtn.addEventListener("click", () => this.indexer());

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

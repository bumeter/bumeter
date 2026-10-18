# Skills API — ghi chú thiết kế (WIP)

> Trạng thái: **chưa viết code**, đang chốt hướng. Ngày 2026-10-17.
> Mục đích: nguồn dữ liệu skill cho **web (bumeter-web)** và **app desktop (Bumeter)**.
> File này là ghi chú làm việc, chưa commit.

---

## 1. Mục tiêu

Web và app đều cần hiển thị/duyệt skill: danh sách, tìm kiếm, chi tiết–preview, và hỗ trợ cài đặt.
Nguồn dữ liệu: **skills.sh** (catalog công khai, không phải của mình).

Ràng buộc không được vi phạm:
- Không telemetry, không upload dữ liệu người dùng.
- App phải chạy được **offline / khi API của mình chết**.
- Không rehost toàn bộ nội dung của người khác ra công khai (license + duplicate content).

---

## 2. Dữ liệu nguồn — đo thật ngày 17/10/2026

| Hạng mục | Kết quả đo |
|---|---|
| Leaderboard | `GET https://skills.sh/api/skills/{hot\|trending\|all-time}/{page}` → **200 skill/trang**, có `total`, `hasMore`, `page` |
| Tổng số skill | all-time **9.908**, trending 9.976, hot 6.868 *(thay đổi liên tục — đo `hot` lúc đầu 4.282, vài phút sau 6.868)* |
| Field trả về (leaderboard) | `source`, `skillId`, `name`, `installs`, `installsYesterday`, `change` — **không có description, không category, `name` thường chỉ là slug** |
| Search | `GET /api/search?q=pdf&limit=5` → `id`, `source`, `skillId`, `name`, `installs` |
| Mô tả skill | `https://raw.githubusercontent.com/<source>/main/skills/<skillId>/SKILL.md` → 200. Fallback: `<id>/SKILL.md`, `.agents/skills/<id>/SKILL.md`, rồi GitHub trees API |
| Kích thước SKILL.md | trung vị **12,6 KB**, max 40 KB → **không lưu cả file**, chỉ lưu `description` + `path` |
| Kích thước index nhẹ | 1.000 skill = 70 KB thô / 15 KB gzip → **9.908 skill ≈ 0,7 MB thô / ~140 KB gzip** (không description) |
| Ngưỡng installs | top 100 ≥ ~199k; top 150 ≥ ~137k; top đầu ~373k |
| `robots.txt` skills.sh | `Disallow: /internal/ /debug-security/ /search /api/` + có sitemap riêng → họ tự làm SEO cho các trang skill |
| Trang skill của họ | render bằng JS, `<title>pdf — skills/anthropics</title>`, meta description template → **nội dung rất mỏng** |

Tự kiểm lại:
```bash
curl -s https://skills.sh/api/skills/all-time/1 | python3 -m json.tool | head -30
curl -s "https://skills.sh/api/search?q=pdf&limit=5" | python3 -m json.tool
curl -sL https://www.skills.sh/robots.txt
curl -s https://raw.githubusercontent.com/anthropics/skills/main/skills/pdf/SKILL.md | head -20
```

**Bẫy đã gặp:** nhiều skill dùng frontmatter block scalar (`description: >`) → regex ngây thơ trả về đúng ký tự `>`. Phải parse YAML thật (PyYAML, hoặc `frontmatter::parse` trong `crates/bum-skills`).

---

## 3. Quyết định đã chốt

1. **Postgres trên VPS** (đã cài sẵn) — không dùng Supabase.
2. **API có version**: `/v1/` để không phá client cũ.
3. **Static-first**: crawl → sinh JSON tĩnh → **nginx serve chính là API**. Chỉ thêm service động sau, cùng path.
4. **App vẫn gọi thẳng skills.sh** (`crates/bum-skills/src/market.rs` đang làm vậy); API của mình là lớp **bổ sung**, không thay thế → gọi có timeout 2–3s, fail thì fallback.
5. **Search chạy client-side** trên index nhẹ (~140 KB gzip), không cần endpoint search.
6. Không lưu SKILL.md, chỉ `description` + `path`.

---

## 4. Kiến trúc

```
skills.sh API ──(cron 1–2 lần/ngày, sleep 0.3–1s)──> VPS
    │
    ├─> Postgres  (skills + skill_installs)   ← nguồn sự thật, history = xu hướng
    │
    └─> generate JSON tĩnh
           └─> nginx  https://api.bumeter.cc/v1/**   ← ĐÂY LÀ API
                      │
                      ├── web (Vercel) : build tĩnh + tải index.json cho search
                      └── app desktop  : tải index.json, cache, conditional GET

App desktop ──> skills.sh trực tiếp (luôn live)  ─┐
            └─> raw.githubusercontent (preview)   ─┼─ app sống được khi API mình chết
            └─> api.bumeter.cc (bổ sung, timeout)  ─┘
```

Không có daemon nào để chết; không expose Postgres; không query DB theo request.
Khi cần search server-side trên description hoặc admin/write → thêm route động **sau cùng path `/v1/`**, client không đổi.

---

## 5. Hợp đồng API v1

```
GET /v1/meta.json                        → {generated_at, total, mode, schema}
GET /v1/skills/index.json                → [[source, skillId, name, installs, change], …]   ~140 KB gzip
GET /v1/skills/<owner>/<repo>/<id>.json  → description, license, stars, path, installs, history 30 ngày
GET /v1/skills/<owner>/<repo>.json       → mọi skill của 1 repo
GET /v1/trending.json                    → top theo installsYesterday / change
GET /healthz
```

Ghi chú:
- `index.json` **không có description** — web/app tải 1 lần, search/sort/filter tại chỗ; description chỉ lấy khi mở chi tiết.
- `ETag` / `Last-Modified` (nginx static cho sẵn) → app conditional GET, data không đổi thì không tải lại.
- `meta.json` để client biết data cũ hay mới.
- CORS cho `https://bumeter.cc`; không auth, không cookie, **không log query** (xem §8).

---

## 6. Schema Postgres

```sql
create table skills (
  id text primary key,               -- 'owner/repo/skillId'
  source text not null,              -- owner/repo
  skill_id text not null,
  name text not null,
  description text,                  -- resolve từ SKILL.md (parse YAML)
  license text,
  stars bigint,
  path text,                         -- skills/<id>/SKILL.md …
  installs bigint not null default 0,
  installs_yesterday bigint,
  first_seen date not null default current_date,
  last_seen  date not null default current_date,
  unique (source, skill_id)
);
create index on skills (installs desc);                    -- leaderboard
create index on skills using gin (to_tsvector('simple',
  coalesce(name,'') || ' ' || coalesce(description,'')));   -- search (nếu làm động)
create extension if not exists pg_trgm;
create index on skills using gin (name gin_trgm_ops);

create table skill_installs (        -- history: 1 dòng/skill/ngày, ghi khi số đổi
  skill_id text references skills(id) on delete cascade,
  day date not null,
  installs bigint not null,
  primary key (skill_id, day)
);
```

Ước lượng dung lượng: `skills` 10k dòng ≈ **~10 MB** (kèm index).
`skill_installs` ghi cả 10k/ngày ≈ 175 MB/năm (~330 MB kèm index); chỉ ghi khi đổi (~2k/ngày) ≈ 70 MB/năm.
Self-host nên không có trần → **ưu tiên ghi đủ để có xu hướng**, gộp tuần sau 90 ngày nếu thấy nặng.

---

## 7. Kế hoạch triển khai

| # | File | Việc |
|---|---|---|
| 1 | `server/skills/schema.sql` | Schema Postgres ở §6 |
| 2 | `server/skills/crawl.py` | skills.sh → Postgres (upsert `(source, skill_id)`, ghi history khi `installs` đổi) + xuất `data/skills.json` |
| 3 | `server/skills/generate.py` | snapshot → `/srv/skills-api/public/v1/**` (index, detail, trending, meta) |
| 4 | `deploy/` | nginx (`/v1/`, gzip, ETag, CORS, rate-limit), cron, systemd timer |

Cron dự kiến:
```bash
# /etc/cron.d/bumeter-skills
17 3 * * * cd /srv/bumeter-skills && .venv/bin/python crawl.py && .venv/bin/python generate.py
```

Stack: **Python cho crawl + generate** (tooling vận hành, không ship cho user).
Không thêm `tokio/axum/sqlx` vào workspace Rust chỉ để chạy một job server-side — app hiện còn không dùng `reqwest`, `market.rs` shell ra `curl`.
Lý do duy nhất để chọn Rust: dùng lại `market.rs` + `frontmatter::parse` đã test (~100 dòng parse, không đáng).

---

## 8. Ràng buộc & rủi ro

- **Privacy:** API thấy IP người dùng → ghi rõ trong privacy note, **không log query**, không auth, không cookie. Chỉ trả catalog công khai.
- **App phải chịu được API chết:** timeout 2–3s + fallback skills.sh + cache local.
- **Postgres:** `listen_addresses = 'localhost'`, **không mở 5432**; user read-only cho generator; `pg_dump` hằng tuần (VPS + snapshot JSON trong repo = 2 bản backup).
- **Lịch sự với skills.sh:** crawl chậm (sleep 0.3–1s), 50 request/lần, cache snapshot, không hammer. `robots.txt` của họ chặn `/api/` → càng phải nhẹ tay.
- **License/duplicate:** description thuộc repo gốc (đa số MIT/Apache) — dùng nội bộ thì không sao; **publish công khai phải attribution + viết lại**, đừng copy hàng loạt.
- **Đừng để Vercel/VPS phụ thuộc nhau chết chéo:** web build từ snapshot đã commit, fail mạng thì dùng bản cũ.

---

## 9. Phương án đã cân nhắc và loại

| Phương án | Lý do loại |
|---|---|
| **Supabase free** làm DB | 500 MB / 5 GB egress / **pause sau 7 ngày không hoạt động** / 500 MB RAM. Dung lượng thì thừa (10 MB) nhưng cái pause là rủi ro vận hành thật. Pro $25/tháng mới hết. VPS đã có Postgres → không cần. |
| **API của mình chen vào giữa app ↔ skills.sh** | Single point of failure, phá pitch "100% local", server thấy query của user. Giữ app gọi thẳng skills.sh. |
| **Rust (axum + sqlx + tokio) trong workspace** | Kéo dependency nặng vào build của app chỉ để chạy job server-side. |
| **SQLite** | Hợp khi data phải nằm trong app; ở đây Postgres đã sẵn và đường lên Supabase về sau chỉ là đổi DSN. |
| **9.908 trang skill cho SEO** | Trang API không có description → thin content, trùng lặp với skills.sh (họ có sitemap riêng). Thay bằng: hub + 100–150 trang tự viết (tiếng Việt) + index tĩnh cho search. |
| **Nhét snapshot enrichment vào bản release** (thay API) | Không phải API → không thấy IP ai, offline vẫn có data, tận dụng auto-update. **Vẫn là phương án thay thế tốt**, cân nhắc nếu không muốn vận hành server nào. |

---

## 10. Câu hỏi cần trả lời trước khi viết code

1. Backend: **static-first (nginx + JSON sinh sẵn)** hay **FastAPI động** ngay từ đầu?
2. Domain: subdomain riêng (`api.bumeter.cc`) hay path trên `bumeter.cc`?
3. Postgres trên VPS: bản nào, đã có database/user chưa? Script đọc **DSN từ env** (không hard-code) — đúng ý chứ?

**Ngoài lề, đang treo:**
- `scripts/downloads.py` (CLI đếm lượt tải theo version) đã tạo, **chưa commit**.
- Commit `e17f31a` (fix test ETXTBSY + test tái hiện) **chưa push** — đang nằm trên nhánh `feat/auto-update`, v1.0.5 đã release.
- Web: bộ đếm lượt tải đã đổi sang "luôn hiện số thật, kể cả 0" và đã deploy (hiện `· 12 downloads`).

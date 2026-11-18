use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "user-http コンポーネント",
    notes: &[
        "account-google-login/http-attempt。start は state とブラウザー値の SHA-256 と nonce・verifier を保存する。callback は一致する試行を消費した後に provider の交換を呼び、新セッションを保存して /me への 303 と Cookie を返す。 未検証範囲。callback の state/code 長境界、provider error、code 欠落、重複ログイン Cookie、保存失敗時の HTTP 応答は未確認。既存セッション Cookie を送ると旧セッションを logout する実装だが、そのローテーションを確認する assertion はない。",
        "account-google-login/provider-authorization。GoogleProvider::authorize は state・nonce とランダムな S256 PKCE challenge/verifier を生成し、openid・profile の認可 URL を返す。",
        "account-google-login/provider-exchange。exchange_code は保存済み PKCE verifier を token endpoint に送り、取得した ID token を verify_response に渡す。 未検証範囲。GoogleProvider::exchange の再ディスカバリー、実 Google 通信、HTTPS 専用クライアントのタイムアウトとリダイレクト拒否をこのテストは通らない。",
        "account-google-login/verified-claims。verify_response はライブラリーの署名・issuer・audience・nonce・期限検証に加えて azp と access-token hash を検証し、空でない subject と固定 Google issuer を返す。 未検証範囲。空 subject の拒否、ID token 欠落、名前がない場合や不正な名前の場合の Player へのフォールバックはこの記録のテストでは直接確認していない。",
        "account-profile/http-rename。rename は serde の未知フィールド拒否を持つ JSON を抽出し、same_origin と DisplayName::new が成功した後、Cookie のハッシュに対応するプロフィールを repository で更新する。 未検証範囲。cross-site の Sec-Fetch-Site、正しい JSON・Origin に対する未認証 PATCH、HTTP を通した 80 文字・制御文字・trim 境界の assertion はない。",
        "account-profile/profile-response。GET /me は Cookie による認証結果を ProfileBody の id・display_name・created_at に変換する。 未検証範囲。created_at の値・型・単位を runtime で独立に確認する assertion はなく、変更前後の全体比較に含まれるだけである。Cookie の認証・期限・重複は account-session が扱う。",
        "account-session/cookie-issuance。set_cookie は Domain を付けず Path=/・HttpOnly・SameSite=Lax・指定 Max-Age を使い、secure 構成では Secure を付ける。BrowserConfig は本番と開発用の Cookie 名を選ぶ。 未検証範囲。本番のセッション Cookie の属性と Max-Age、ログアウトの Max-Age=0 を直接比較する assertion はない。ログイン Cookie と共有する helper の証拠を本番セッションの直接検証と扱わない。",
        "account-session/http-logout。logout は Origin の完全一致と、存在する場合の Sec-Fetch-Site: same-origin を要求する。有効な形式の Cookie は repository から削除し、欠落や不正形式でも Cookie を消去して 204 を返す。 未検証範囲。ログアウトの hostile Origin、cross-site の Sec-Fetch-Site、不正形式 Cookie、repository 削除失敗を直接確認する assertion はない。",
        "account-session/session-cookie-input。user HTTP の cookie は対象 Cookie を全 Cookie ヘッダーから探し、重複・非 hex・64 文字以外・欠落を Unauthorized とする。GET /me は受け取った値の SHA-256 だけを repository に渡す。 未検証範囲。user HTTP 自体の複数 Cookie ヘッダー、非 hex と長さ境界は直接確認していない。realtime の Cookie 検証は別実装である。",
        "public-api-documents/contract-user-schema。ユーザーサービスのルートから生成する OpenAPI は、サービスの識別情報、過不足のない六つの操作、Cookie セキュリティ方式の選択肢、要求と応答のスキーマを持つ。 未検証範囲。実行時の入力拒否の事例は、すべての応答種別や成功・エラーの全ヘッダーを網羅しない。スキーマの検査では外部認証プロバイダーを利用できることは確認しない。",
        "public-api-documents/user-runtime。ユーザーサービスの文書ルートは、保存層や外部認証プロバイダーを呼ばずにオフラインの OpenAPI を返し、非公開応答用のヘッダーを付ける。",
        "startup-schema-isolation/account-environment。ServerConfig::from_env は空でない公開オリジン、PostgreSQL URL、Google 認証情報を必須とする。セッション有効期間を 60 秒から 2592000 秒に制限し、待ち受けアドレスを解析し、開発用フラグは true/false だけを受け入れる。 未検証範囲。BrowserConfig のコンストラクターテストは from_env を呼ばない。必須の環境変数、不正な URL プレフィックス、セッション有効期間の境界、待ち受けアドレスの解析、開発用フラグの解析と既定値を、環境を書き換えて検証する対応テストはない。",
        "startup-schema-isolation/contract-browser-config。BrowserConfig はスキーム、ループバックの明示指定、認証情報、パス、クエリー、フラグメントを検証し、正規化したオリジンから callback_url を導出する。 未検証範囲。ServerConfig::from_env の必須条件、PostgreSQL URL の検証、セッション TTL、待ち受けアドレスの解析、不正な開発用フラグは、このコンストラクターテストでは直接確認していない。",
        "account-google-login/browser-bound。ログイン試行はブラウザーに結び付き、有効期限を過ぎた試行や使用済みの試行はコード交換に使えない。",
        "account-google-login/pkce。認可要求は openid・profile と state・nonce・S256 PKCE を使い、コード交換は保存した verifier を送る。",
        "account-google-login/verified-identity。Google の署名、issuer、audience、nonce、有効期限、authorized party と、存在する場合の access-token hash を検証してから issuer・subject を本人識別に使う。",
        "account-google-login/atomic-account-session。同じ issuer・subject の同時初回ログインは一つのアカウントに収束し、新しい本人識別情報とセッションはまとめて保存される。",
        "account-profile/own-profile。認証された自分のプロフィールを id・display_name・created_at として返す。内部 ID は空でなく 128 UTF-8 バイト以内で Unicode 空白を含まない。",
        "account-profile/browser-input。表示名変更には同一 Origin と認証済みセッションが必要で、不正な JSON や未知フィールドは拒否する。",
        "account-profile/name-boundaries。表示名は前後の空白を除いた後、1〜80 Unicode 文字で制御文字を含まない値だけを受け入れる。",
        "account-profile/durable-profile。変更した表示名は store の再接続と後続の同じ本人識別によるログインで維持される。",
        "account-session/opaque-authentication。GET /me は一意な 64 文字の hex Cookie を要求し、そのハッシュに一致する有効期限内のセッションだけを認証する。",
        "account-session/durable-revocation。同一 Origin からのログアウトは指定セッションを失効させる。失効は store の再接続後も維持され、別セッションは引き続き使える。",
        "account-session/browser-cookie-policy。発行 Cookie は Path=/・HttpOnly・SameSite=Lax を使い、本番 HTTPS ではホスト専用 __Host- 名と Secure を使う。",
        "account-session/consumer-revalidation。realtime の認証は /me が返す本人識別を使い、失効または期限切れを再確認した接続ではその後のコマンドや通知を継続しない。"
    ],
    external: &[],
    operations: &[
        trace::Operation {
            contract: "user-http/frontend-get-auth-google-start",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "GET /auth/google/start",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "user-http/frontend-get-auth-google-callback",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "GET /auth/google/callback",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "user-http/frontend-get-me",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "GET /me",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "user-http/frontend-patch-me",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "PATCH /me",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "user-http/frontend-post-auth-logout",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "POST /auth/logout",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "user-http/frontend-get-openapi-json",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "user-openapi",
                operation: "GET /openapi.json",
                package: "user-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        }
    ],
    entrypoints: &[]
});

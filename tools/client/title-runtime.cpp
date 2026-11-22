// The patched client gets custom text from Kryta. The world owns all progress.
#include <windows.h>
#include <shellapi.h>
#include <winhttp.h>

#include <cstdint>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

namespace {
constexpr wchar_t path[] = L"/kryta/custom-titles.json";
constexpr uint32_t first_custom_title_row = 48; // Build 39167 Char title metadata lookup.

struct Http {
    HINTERNET handle;
    ~Http() { if (handle) WinHttpCloseHandle(handle); }
};

std::wstring default_url() {
    int count = 0;
    auto args = CommandLineToArgvW(GetCommandLineW(), &count);
    std::wstring url;
    if (args) {
        for (int i = 1; i + 1 < count; ++i) {
            if (std::wstring(args[i]) == L"-portal") {
                // The stock portal uses port 6601 (docs/decisions/0003-portal-host.md).
                url = L"http://" + std::wstring(args[i + 1]) + L":6601" + path;
                break;
            }
        }
        LocalFree(args);
    }
    return url;
}

std::string fetch(const std::wstring& url) {
    URL_COMPONENTS parts{};
    parts.dwStructSize = sizeof parts;
    parts.dwHostNameLength = parts.dwUrlPathLength = DWORD(-1);
    if (url.empty() || !WinHttpCrackUrl(url.c_str(), 0, 0, &parts)
        || (parts.nScheme != INTERNET_SCHEME_HTTP && parts.nScheme != INTERNET_SCHEME_HTTPS))
        return {};
    std::wstring host(parts.lpszHostName, parts.dwHostNameLength);
    std::wstring target(parts.lpszUrlPath, parts.dwUrlPathLength);
    Http session{WinHttpOpen(L"KrytaTitles/1", WINHTTP_ACCESS_TYPE_NO_PROXY, nullptr, nullptr, 0)};
    if (!session.handle)
        return {};
    WinHttpSetTimeouts(session.handle, 1500, 1500, 1500, 1500);
    Http server{WinHttpConnect(session.handle, host.c_str(), parts.nPort, 0)};
    if (!server.handle)
        return {};
    Http request{WinHttpOpenRequest(server.handle, L"GET", target.c_str(), nullptr, nullptr,
                                   nullptr, parts.nScheme == INTERNET_SCHEME_HTTPS ? WINHTTP_FLAG_SECURE : 0)};
    if (!request.handle)
        return {};
    DWORD redirects = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
    if (!WinHttpSetOption(request.handle, WINHTTP_OPTION_REDIRECT_POLICY, &redirects, sizeof redirects)
        || !WinHttpSendRequest(request.handle, nullptr, 0, nullptr, 0, 0, 0)
        || !WinHttpReceiveResponse(request.handle, nullptr))
        return {};
    DWORD status = 0, bytes = sizeof status;
    if (!WinHttpQueryHeaders(request.handle, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                             nullptr, &status, &bytes, nullptr) || status != 200)
        return {};
    std::string body;
    char buffer[4096];
    for (;;) {
        DWORD read = 0;
        if (!WinHttpReadData(request.handle, buffer, sizeof buffer, &read))
            return {};
        if (!read)
            return body;
        if (body.size() + read > 65536)
            return {};
        body.append(buffer, read);
    }
}

std::wstring coded(const std::string& text) {
    if (text.empty() || text.find('\0') != std::string::npos)
        throw std::runtime_error("invalid title text");
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                    static_cast<int>(text.size()), nullptr, 0);
    if (!count)
        throw std::runtime_error("invalid title UTF-8");
    std::wstring utf16(count, L'\0');
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                        static_cast<int>(text.size()), utf16.data(), count);
    // Build 39167 TextEncode: %str1%, literal tag, escaped text, terminator.
    std::wstring result{0x0BA9, 0x0107};
    for (wchar_t c : utf16) {
        if (c == L'[' || c == L']' || c == L'\\')
            result.push_back(L'\\');
        result.push_back(c);
    }
    result.push_back(1);
    return result;
}

struct Title {
    uint32_t id, rank, next_rank, goal;
    std::wstring fallback, name, description;
};

struct Catalog {
    std::mutex mutex;
    ULONGLONG retry_after = 0;
    std::vector<Title> rows;

    void refresh(const wchar_t* override_url) {
        const auto now = GetTickCount64();
        if (now < retry_after)
            return;
        retry_after = now + 3000;
        const auto body = fetch(*override_url ? std::wstring(override_url) : default_url());
        if (body.empty())
            return;
        const auto json = nlohmann::json::parse(body);
        std::vector<Title> next;
        for (const auto& row : json.at("custom_tracks")) {
            const auto id = row.at("id").get<uint32_t>();
            if (id < first_custom_title_row)
                continue;
            const auto units = row.at("short_units").get<std::vector<uint16_t>>();
            if (units.size() < 4 || units.size() > 7 || units[0] != 0x0BA9 || units[1] != 0x0107)
                continue;
            next.push_back({
                id, row.at("current_rank").get<uint32_t>(),
                row.at("next_rank").get<uint32_t>(), row.at("next_minimum").get<uint32_t>(),
                std::wstring(units.begin(), units.end()),
                coded(row.at("name").get<std::string>()),
                coded(row.at("description").get<std::string>()),
            });
        }
        rows = std::move(next);
    }
};

Catalog& catalog() {
    static Catalog value;
    return value;
}
}

extern "C" __declspec(dllexport) const wchar_t* __cdecl kryta_title_name(
    const wchar_t* url, uint32_t rank, const wchar_t* original) noexcept {
    if (!original || original[0] != 0x0BA9 || original[1] != 0x0107)
        return nullptr;
    try {
        auto& data = catalog();
        std::lock_guard guard(data.mutex);
        data.refresh(url);
        for (const auto& row : data.rows) {
            if (row.rank == rank && row.fallback == original) {
                thread_local std::wstring text;
                text = row.name;
                return text.c_str();
            }
        }
    } catch (...) {
        // Keep using stock text when the catalog is unavailable.
    }
    return nullptr;
}

extern "C" __declspec(dllexport) const wchar_t* __cdecl kryta_title_hover_v2(
    const wchar_t*, uint32_t id, uint32_t rank, uint32_t next_rank, uint32_t goal) noexcept {
    if (id < first_custom_title_row)
        return nullptr;
    try {
        auto& data = catalog();
        std::lock_guard guard(data.mutex);
        for (const auto& row : data.rows) {
            if (row.id == id && row.rank == rank && row.next_rank == next_rank && row.goal == goal) {
                thread_local std::wstring text;
                text = row.description;
                return text.c_str();
            }
        }
    } catch (...) {
        // Keep using stock text until a matching custom definition arrives.
    }
    return nullptr;
}

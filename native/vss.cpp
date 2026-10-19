// Small Windows SDK bridge: IVssBackupComponents is not in windows-rs metadata.
// All COM calls and releases stay on the same dedicated Rust-owned thread.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <vss.h>
#include <vswriter.h>
#include <vsbackup.h>


struct Session {
    IVssBackupComponents* components;
    VSS_ID set;
    bool com;
    HRESULT close() {
        HRESULT result = S_OK;
        if (components) {
            if (set != GUID_NULL) {
                LONG deleted = 0;
                VSS_ID remaining = GUID_NULL;
                result = components->DeleteSnapshots(set, VSS_OBJECT_SNAPSHOT_SET,
                                                     FALSE, &deleted, &remaining);
                if (result == VSS_E_OBJECT_NOT_FOUND) result = S_OK;
            }
            components->Release(); // Nonpersistent auto-release context, also on failure.
            components = nullptr;
        }
        if (com) { CoUninitialize(); com = false; }
        return result;
    }

};

static HRESULT dispose(Session* session) {
    HRESULT result = session->close();
    HeapFree(GetProcessHeap(), 0, session);
    return result;
}

extern "C" int straja_vss_admin() {
    SID_IDENTIFIER_AUTHORITY authority = SECURITY_NT_AUTHORITY;
    PSID administrators = nullptr;
    BOOL member = FALSE;
    if (AllocateAndInitializeSid(&authority, 2, SECURITY_BUILTIN_DOMAIN_RID,
        DOMAIN_ALIAS_RID_ADMINS, 0, 0, 0, 0, 0, 0, &administrators)) {
        CheckTokenMembership(nullptr, administrators, &member);
        FreeSid(administrators);
    }
    return member != FALSE;
}

extern "C" HRESULT straja_vss_volume(const wchar_t* source, wchar_t* root,
                                     wchar_t* volume, DWORD capacity) {
    if (!GetVolumePathNameW(source, root, capacity)) return HRESULT_FROM_WIN32(GetLastError());
    // Initial scope: fixed, local NTFS volumes. No mapped drives or mounted folders.
    if (lstrlenW(root) != 3 || root[1] != L':' || GetDriveTypeW(root) != DRIVE_FIXED)
        return HRESULT_FROM_WIN32(ERROR_NOT_SUPPORTED);
    wchar_t filesystem[32] = {};
    if (!GetVolumeInformationW(root, nullptr, 0, nullptr, nullptr, nullptr, filesystem, 32))
        return HRESULT_FROM_WIN32(GetLastError());
    if (CompareStringOrdinal(filesystem, -1, L"NTFS", -1, TRUE) != CSTR_EQUAL) return HRESULT_FROM_WIN32(ERROR_NOT_SUPPORTED);
    if (!GetVolumeNameForVolumeMountPointW(root, volume, capacity))
        return HRESULT_FROM_WIN32(GetLastError());
    return S_OK;
}

// MULTI_SZ list of mounted folders, so Restic cannot silently read an unsupported
// mounted volume live when its VSS wrapper falls back to the original path.
extern "C" HRESULT straja_vss_mounts(const wchar_t* root, wchar_t* result, DWORD capacity) {
    if (capacity < 2) return E_INVALIDARG;
    result[0] = 0;
    wchar_t path[32768] = {};
    HANDLE search = FindFirstVolumeMountPointW(root, path, 32768);
    if (search == INVALID_HANDLE_VALUE) {
        DWORD error = GetLastError();
        return error == ERROR_NO_MORE_FILES ? S_OK : HRESULT_FROM_WIN32(error);
    }
    DWORD used = 0;
    HRESULT hr = S_OK;
    do {
        size_t length = static_cast<size_t>(lstrlenW(path));
        if (used + length + 2 > capacity) { hr = HRESULT_FROM_WIN32(ERROR_MORE_DATA); break; }
        CopyMemory(result + used, path, (length + 1) * sizeof(wchar_t));
        used += static_cast<DWORD>(length) + 1;
        result[used] = 0;
        if (!FindNextVolumeMountPointW(search, path, 32768)) {
            DWORD error = GetLastError();
            if (error != ERROR_NO_MORE_FILES) hr = HRESULT_FROM_WIN32(error);
            break;
        }
    } while (true);
    FindVolumeMountPointClose(search);
    return hr;
}

extern "C" HRESULT straja_vss_begin(const wchar_t* const* volumes, DWORD count,
                                    wchar_t* devices, DWORD capacity, void** output) {
    *output = nullptr;
    if (count == 0 || count > 100 || capacity < MAX_PATH) return E_INVALIDARG;
    Session* session = static_cast<Session*>(HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof(Session)));
    if (!session) return E_OUTOFMEMORY;
    HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (SUCCEEDED(hr)) session->com = true;
    if (SUCCEEDED(hr)) {
        hr = CoInitializeSecurity(nullptr, -1, nullptr, nullptr, RPC_C_AUTHN_LEVEL_PKT_PRIVACY,
                                  RPC_C_IMP_LEVEL_IMPERSONATE, nullptr, EOAC_NONE, nullptr);
        if (hr == RPC_E_TOO_LATE) hr = S_OK;
    }
    if (SUCCEEDED(hr)) hr = CreateVssBackupComponents(&session->components);
    if (SUCCEEDED(hr)) hr = session->components->InitializeForBackup();
    // File-level, crash-consistent snapshot. No writers, log truncation, or backup-history changes.
    if (SUCCEEDED(hr)) hr = session->components->SetContext(VSS_CTX_FILE_SHARE_BACKUP);
    if (SUCCEEDED(hr)) hr = session->components->SetBackupState(FALSE, FALSE, VSS_BT_COPY, FALSE);
    if (SUCCEEDED(hr)) hr = session->components->StartSnapshotSet(&session->set);
    VSS_ID snapshots[100] = {};
    for (DWORD i = 0; SUCCEEDED(hr) && i < count; ++i) {
        BOOL supported = FALSE;
        hr = session->components->IsVolumeSupported(GUID_NULL, const_cast<wchar_t*>(volumes[i]), &supported);
        if (SUCCEEDED(hr) && !supported) hr = VSS_E_VOLUME_NOT_SUPPORTED;
        if (SUCCEEDED(hr)) hr = session->components->AddToSnapshotSet(
            const_cast<wchar_t*>(volumes[i]), GUID_NULL, &snapshots[i]);
    }
    IVssAsync* operation = nullptr;
    if (SUCCEEDED(hr)) hr = session->components->DoSnapshotSet(&operation);
    if (SUCCEEDED(hr)) {
        const ULONGLONG deadline = GetTickCount64() + 120000;
        HRESULT status = VSS_S_ASYNC_PENDING;
        while (SUCCEEDED(hr) && status == VSS_S_ASYNC_PENDING && GetTickCount64() < deadline) {
            hr = operation->Wait(250);
            if (SUCCEEDED(hr)) hr = operation->QueryStatus(&status, nullptr);
        }
        if (SUCCEEDED(hr)) hr = status == VSS_S_ASYNC_FINISHED ? S_OK :
            (FAILED(status) ? status : HRESULT_FROM_WIN32(ERROR_TIMEOUT));
        if (FAILED(hr)) { operation->Cancel(); session->components->AbortBackup(); }
    }
    if (operation) operation->Release();
    for (DWORD i = 0; SUCCEEDED(hr) && i < count; ++i) {
        VSS_SNAPSHOT_PROP properties = {};
        hr = session->components->GetSnapshotProperties(snapshots[i], &properties);
        if (SUCCEEDED(hr)) {
            if (!properties.m_pwszSnapshotDeviceObject ||
                static_cast<DWORD>(lstrlenW(properties.m_pwszSnapshotDeviceObject)) >= capacity) hr = E_UNEXPECTED;
            else CopyMemory(devices + i * capacity, properties.m_pwszSnapshotDeviceObject,
                (static_cast<size_t>(lstrlenW(properties.m_pwszSnapshotDeviceObject)) + 1) * sizeof(wchar_t));
            VssFreeSnapshotProperties(&properties);
        }
    }
    if (FAILED(hr)) { dispose(session); return hr; }
    *output = session;
    return S_OK;
}

extern "C" HRESULT straja_vss_end(void* handle) {
    Session* session = static_cast<Session*>(handle);
    if (!session) return E_INVALIDARG;
    return dispose(session);
}

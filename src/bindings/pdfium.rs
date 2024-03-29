#![allow(warnings, clippy::all, unused)]

use windows::Win32::Graphics::Gdi::HDC;
/*
    generated by rust-bindgen 0.59.1
    cmdline:
        bindgen fpdfview.h -o fpdfview.rs
        --no-layout-tests
        --use-core
        --size_t-is-usize
        --allowlist-function "F?PDF.*"
        --allowlist-type "(F?PDF|FX_).*"
        --allowlist-var "F?PDF.*"
        --blocklist-type "HDC|HDC__"
        --no-copy "fpdf_.*_t__"
        --no-debug "fpdf_.*_t__"
        --no-copy "fpdf_.*_t__"
        --default-macro-constant-type signed
*/

pub const FPDF_OBJECT_UNKNOWN: i32 = 0;
pub const FPDF_OBJECT_BOOLEAN: i32 = 1;
pub const FPDF_OBJECT_NUMBER: i32 = 2;
pub const FPDF_OBJECT_STRING: i32 = 3;
pub const FPDF_OBJECT_NAME: i32 = 4;
pub const FPDF_OBJECT_ARRAY: i32 = 5;
pub const FPDF_OBJECT_DICTIONARY: i32 = 6;
pub const FPDF_OBJECT_STREAM: i32 = 7;
pub const FPDF_OBJECT_NULLOBJ: i32 = 8;
pub const FPDF_OBJECT_REFERENCE: i32 = 9;
pub const FPDF_POLICY_MACHINETIME_ACCESS: i32 = 0;
pub const FPDF_ERR_SUCCESS: i32 = 0;
pub const FPDF_ERR_UNKNOWN: i32 = 1;
pub const FPDF_ERR_FILE: i32 = 2;
pub const FPDF_ERR_FORMAT: i32 = 3;
pub const FPDF_ERR_PASSWORD: i32 = 4;
pub const FPDF_ERR_SECURITY: i32 = 5;
pub const FPDF_ERR_PAGE: i32 = 6;
pub const FPDF_ANNOT: i32 = 1;
pub const FPDF_LCD_TEXT: i32 = 2;
pub const FPDF_NO_NATIVETEXT: i32 = 4;
pub const FPDF_GRAYSCALE: i32 = 8;
pub const FPDF_DEBUG_INFO: i32 = 128;
pub const FPDF_NO_CATCH: i32 = 256;
pub const FPDF_RENDER_LIMITEDIMAGECACHE: i32 = 512;
pub const FPDF_RENDER_FORCEHALFTONE: i32 = 1024;
pub const FPDF_PRINTING: i32 = 2048;
pub const FPDF_RENDER_NO_SMOOTHTEXT: i32 = 4096;
pub const FPDF_RENDER_NO_SMOOTHIMAGE: i32 = 8192;
pub const FPDF_RENDER_NO_SMOOTHPATH: i32 = 16384;
pub const FPDF_REVERSE_BYTE_ORDER: i32 = 16;
pub const FPDF_CONVERT_FILL_TO_STROKE: i32 = 32;
pub const FPDFBitmap_Unknown: i32 = 0;
pub const FPDFBitmap_Gray: i32 = 1;
pub const FPDFBitmap_BGR: i32 = 2;
pub const FPDFBitmap_BGRx: i32 = 3;
pub const FPDFBitmap_BGRA: i32 = 4;
pub type wchar_t = ::std::os::raw::c_ushort;
pub type BYTE = ::std::os::raw::c_uchar;
pub type CHAR = ::std::os::raw::c_char;
pub type LONG = ::std::os::raw::c_long;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct tagLOGFONTA {
    pub lfHeight: LONG,
    pub lfWidth: LONG,
    pub lfEscapement: LONG,
    pub lfOrientation: LONG,
    pub lfWeight: LONG,
    pub lfItalic: BYTE,
    pub lfUnderline: BYTE,
    pub lfStrikeOut: BYTE,
    pub lfCharSet: BYTE,
    pub lfOutPrecision: BYTE,
    pub lfClipPrecision: BYTE,
    pub lfQuality: BYTE,
    pub lfPitchAndFamily: BYTE,
    pub lfFaceName: [CHAR; 32usize],
}
pub type LOGFONTA = tagLOGFONTA;
pub type LOGFONT = LOGFONTA;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_UNKNOWN: FPDF_TEXT_RENDERMODE = -1;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_FILL: FPDF_TEXT_RENDERMODE = 0;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_STROKE: FPDF_TEXT_RENDERMODE = 1;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_FILL_STROKE: FPDF_TEXT_RENDERMODE = 2;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_INVISIBLE: FPDF_TEXT_RENDERMODE = 3;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_FILL_CLIP: FPDF_TEXT_RENDERMODE = 4;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_STROKE_CLIP: FPDF_TEXT_RENDERMODE = 5;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_FILL_STROKE_CLIP: FPDF_TEXT_RENDERMODE = 6;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_CLIP: FPDF_TEXT_RENDERMODE = 7;
pub const FPDF_TEXT_RENDERMODE_FPDF_TEXTRENDERMODE_LAST: FPDF_TEXT_RENDERMODE = 7;
pub type FPDF_TEXT_RENDERMODE = ::std::os::raw::c_int;
#[repr(C)]
pub struct fpdf_action_t__ {
    _unused: [u8; 0],
}
pub type FPDF_ACTION = *mut fpdf_action_t__;
#[repr(C)]
pub struct fpdf_annotation_t__ {
    _unused: [u8; 0],
}
pub type FPDF_ANNOTATION = *mut fpdf_annotation_t__;
#[repr(C)]
pub struct fpdf_attachment_t__ {
    _unused: [u8; 0],
}
pub type FPDF_ATTACHMENT = *mut fpdf_attachment_t__;
#[repr(C)]
pub struct fpdf_avail_t__ {
    _unused: [u8; 0],
}
pub type FPDF_AVAIL = *mut fpdf_avail_t__;
#[repr(C)]
pub struct fpdf_bitmap_t__ {
    _unused: [u8; 0],
}
pub type FPDF_BITMAP = *mut fpdf_bitmap_t__;
#[repr(C)]
pub struct fpdf_bookmark_t__ {
    _unused: [u8; 0],
}
pub type FPDF_BOOKMARK = *mut fpdf_bookmark_t__;
#[repr(C)]
pub struct fpdf_clippath_t__ {
    _unused: [u8; 0],
}
pub type FPDF_CLIPPATH = *mut fpdf_clippath_t__;
#[repr(C)]
pub struct fpdf_dest_t__ {
    _unused: [u8; 0],
}
pub type FPDF_DEST = *mut fpdf_dest_t__;
#[repr(C)]
pub struct fpdf_document_t__ {
    _unused: [u8; 0],
}
pub type FPDF_DOCUMENT = *mut fpdf_document_t__;
#[repr(C)]
pub struct fpdf_font_t__ {
    _unused: [u8; 0],
}
pub type FPDF_FONT = *mut fpdf_font_t__;
#[repr(C)]
pub struct fpdf_form_handle_t__ {
    _unused: [u8; 0],
}
pub type FPDF_FORMHANDLE = *mut fpdf_form_handle_t__;
#[repr(C)]
pub struct fpdf_glyphpath_t__ {
    _unused: [u8; 0],
}
pub type FPDF_GLYPHPATH = *const fpdf_glyphpath_t__;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fpdf_javascript_action_t {
    _unused: [u8; 0],
}
pub type FPDF_JAVASCRIPT_ACTION = *mut fpdf_javascript_action_t;
#[repr(C)]
pub struct fpdf_link_t__ {
    _unused: [u8; 0],
}
pub type FPDF_LINK = *mut fpdf_link_t__;
#[repr(C)]
pub struct fpdf_page_t__ {
    _unused: [u8; 0],
}
pub type FPDF_PAGE = *mut fpdf_page_t__;
#[repr(C)]
pub struct fpdf_pagelink_t__ {
    _unused: [u8; 0],
}
pub type FPDF_PAGELINK = *mut fpdf_pagelink_t__;
#[repr(C)]
pub struct fpdf_pageobject_t__ {
    _unused: [u8; 0],
}
pub type FPDF_PAGEOBJECT = *mut fpdf_pageobject_t__;
#[repr(C)]
pub struct fpdf_pageobjectmark_t__ {
    _unused: [u8; 0],
}
pub type FPDF_PAGEOBJECTMARK = *mut fpdf_pageobjectmark_t__;
#[repr(C)]
pub struct fpdf_pagerange_t__ {
    _unused: [u8; 0],
}
pub type FPDF_PAGERANGE = *mut fpdf_pagerange_t__;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fpdf_pathsegment_t {
    _unused: [u8; 0],
}
pub type FPDF_PATHSEGMENT = *const fpdf_pathsegment_t;
pub type FPDF_RECORDER = *mut ::core::ffi::c_void;
#[repr(C)]
pub struct fpdf_schhandle_t__ {
    _unused: [u8; 0],
}
pub type FPDF_SCHHANDLE = *mut fpdf_schhandle_t__;
#[repr(C)]
pub struct fpdf_signature_t__ {
    _unused: [u8; 0],
}
pub type FPDF_SIGNATURE = *mut fpdf_signature_t__;
#[repr(C)]
pub struct fpdf_structelement_t__ {
    _unused: [u8; 0],
}
pub type FPDF_STRUCTELEMENT = *mut fpdf_structelement_t__;
#[repr(C)]
pub struct fpdf_structtree_t__ {
    _unused: [u8; 0],
}
pub type FPDF_STRUCTTREE = *mut fpdf_structtree_t__;
#[repr(C)]
pub struct fpdf_textpage_t__ {
    _unused: [u8; 0],
}
pub type FPDF_TEXTPAGE = *mut fpdf_textpage_t__;
#[repr(C)]
pub struct fpdf_widget_t__ {
    _unused: [u8; 0],
}
pub type FPDF_WIDGET = *mut fpdf_widget_t__;
#[repr(C)]
pub struct fpdf_xobject_t__ {
    _unused: [u8; 0],
}
pub type FPDF_XOBJECT = *mut fpdf_xobject_t__;
pub type FPDF_BOOL = ::std::os::raw::c_int;
pub type FPDF_RESULT = ::std::os::raw::c_int;
pub type FPDF_DWORD = ::std::os::raw::c_ulong;
pub const _FPDF_DUPLEXTYPE__DuplexUndefined: _FPDF_DUPLEXTYPE_ = 0;
pub const _FPDF_DUPLEXTYPE__Simplex: _FPDF_DUPLEXTYPE_ = 1;
pub const _FPDF_DUPLEXTYPE__DuplexFlipShortEdge: _FPDF_DUPLEXTYPE_ = 2;
pub const _FPDF_DUPLEXTYPE__DuplexFlipLongEdge: _FPDF_DUPLEXTYPE_ = 3;
pub type _FPDF_DUPLEXTYPE_ = ::std::os::raw::c_int;
pub use self::_FPDF_DUPLEXTYPE_ as FPDF_DUPLEXTYPE;
pub type FPDF_WCHAR = ::std::os::raw::c_ushort;
pub type FPDF_BYTESTRING = *const ::std::os::raw::c_char;
pub type FPDF_WIDESTRING = *const ::std::os::raw::c_ushort;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FPDF_BSTR_ {
    pub str_: *mut ::std::os::raw::c_char,
    pub len: ::std::os::raw::c_int,
}
pub type FPDF_BSTR = FPDF_BSTR_;
pub type FPDF_STRING = *const ::std::os::raw::c_char;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _FS_MATRIX_ {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}
pub type FS_MATRIX = _FS_MATRIX_;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _FS_RECTF_ {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
pub type FS_RECTF = _FS_RECTF_;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FS_SIZEF_ {
    pub width: f32,
    pub height: f32,
}
pub type FS_SIZEF = FS_SIZEF_;
pub type FPDF_ANNOTATION_SUBTYPE = ::std::os::raw::c_int;
pub type FPDF_ANNOT_APPEARANCEMODE = ::std::os::raw::c_int;
pub type FPDF_OBJECT_TYPE = ::std::os::raw::c_int;
extern "C" {
    pub fn FPDF_InitLibrary();
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FPDF_LIBRARY_CONFIG_ {
    pub version: ::std::os::raw::c_int,
    pub m_pUserFontPaths: *mut *const ::std::os::raw::c_char,
    pub m_pIsolate: *mut ::core::ffi::c_void,
    pub m_v8EmbedderSlot: ::std::os::raw::c_uint,
    pub m_pPlatform: *mut ::core::ffi::c_void,
}
pub type FPDF_LIBRARY_CONFIG = FPDF_LIBRARY_CONFIG_;
extern "C" {
    pub fn FPDF_InitLibraryWithConfig(config: *const FPDF_LIBRARY_CONFIG);
}
extern "C" {
    pub fn FPDF_DestroyLibrary();
}
extern "C" {
    pub fn FPDF_SetSandBoxPolicy(policy: FPDF_DWORD, enable: FPDF_BOOL);
}
pub type PDFiumEnsureTypefaceCharactersAccessible = ::core::option::Option<
    unsafe extern "C" fn(font: *const LOGFONT, text: *const wchar_t, text_length: usize),
>;
extern "C" {
    pub fn FPDF_SetTypefaceAccessibleFunc(func: PDFiumEnsureTypefaceCharactersAccessible);
}
extern "C" {
    pub fn FPDF_SetPrintTextWithGDI(use_gdi: FPDF_BOOL);
}
extern "C" {
    pub fn FPDF_SetPrintMode(mode: ::std::os::raw::c_int) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_LoadDocument(file_path: FPDF_STRING, password: FPDF_BYTESTRING) -> FPDF_DOCUMENT;
}
extern "C" {
    pub fn FPDF_LoadMemDocument(
        data_buf: *const ::core::ffi::c_void,
        size: ::std::os::raw::c_int,
        password: FPDF_BYTESTRING,
    ) -> FPDF_DOCUMENT;
}
extern "C" {
    pub fn FPDF_LoadMemDocument64(
        data_buf: *const ::core::ffi::c_void,
        size: usize,
        password: FPDF_BYTESTRING,
    ) -> FPDF_DOCUMENT;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FPDF_FILEACCESS {
    pub m_FileLen: ::std::os::raw::c_ulong,
    pub m_GetBlock: ::core::option::Option<
        unsafe extern "C" fn(
            param: *mut ::core::ffi::c_void,
            position: ::std::os::raw::c_ulong,
            pBuf: *mut ::std::os::raw::c_uchar,
            size: ::std::os::raw::c_ulong,
        ) -> ::std::os::raw::c_int,
    >,
    pub m_Param: *mut ::core::ffi::c_void,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FPDF_FILEHANDLER_ {
    pub clientData: *mut ::core::ffi::c_void,
    pub Release: ::core::option::Option<unsafe extern "C" fn(clientData: *mut ::core::ffi::c_void)>,
    pub GetSize: ::core::option::Option<
        unsafe extern "C" fn(clientData: *mut ::core::ffi::c_void) -> FPDF_DWORD,
    >,
    pub ReadBlock: ::core::option::Option<
        unsafe extern "C" fn(
            clientData: *mut ::core::ffi::c_void,
            offset: FPDF_DWORD,
            buffer: *mut ::core::ffi::c_void,
            size: FPDF_DWORD,
        ) -> FPDF_RESULT,
    >,
    pub WriteBlock: ::core::option::Option<
        unsafe extern "C" fn(
            clientData: *mut ::core::ffi::c_void,
            offset: FPDF_DWORD,
            buffer: *const ::core::ffi::c_void,
            size: FPDF_DWORD,
        ) -> FPDF_RESULT,
    >,
    pub Flush: ::core::option::Option<
        unsafe extern "C" fn(clientData: *mut ::core::ffi::c_void) -> FPDF_RESULT,
    >,
    pub Truncate: ::core::option::Option<
        unsafe extern "C" fn(clientData: *mut ::core::ffi::c_void, size: FPDF_DWORD) -> FPDF_RESULT,
    >,
}
pub type FPDF_FILEHANDLER = FPDF_FILEHANDLER_;
extern "C" {
    pub fn FPDF_LoadCustomDocument(
        pFileAccess: *mut FPDF_FILEACCESS,
        password: FPDF_BYTESTRING,
    ) -> FPDF_DOCUMENT;
}
extern "C" {
    pub fn FPDF_GetFileVersion(
        doc: FPDF_DOCUMENT,
        fileVersion: *mut ::std::os::raw::c_int,
    ) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_GetLastError() -> ::std::os::raw::c_ulong;
}
extern "C" {
    pub fn FPDF_DocumentHasValidCrossReferenceTable(document: FPDF_DOCUMENT) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_GetTrailerEnds(
        document: FPDF_DOCUMENT,
        buffer: *mut ::std::os::raw::c_uint,
        length: ::std::os::raw::c_ulong,
    ) -> ::std::os::raw::c_ulong;
}
extern "C" {
    pub fn FPDF_GetDocPermissions(document: FPDF_DOCUMENT) -> ::std::os::raw::c_ulong;
}
extern "C" {
    pub fn FPDF_GetSecurityHandlerRevision(document: FPDF_DOCUMENT) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDF_GetPageCount(document: FPDF_DOCUMENT) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDF_LoadPage(document: FPDF_DOCUMENT, page_index: ::std::os::raw::c_int) -> FPDF_PAGE;
}
extern "C" {
    pub fn FPDF_GetPageWidthF(page: FPDF_PAGE) -> f32;
}
extern "C" {
    pub fn FPDF_GetPageWidth(page: FPDF_PAGE) -> f64;
}
extern "C" {
    pub fn FPDF_GetPageHeightF(page: FPDF_PAGE) -> f32;
}
extern "C" {
    pub fn FPDF_GetPageHeight(page: FPDF_PAGE) -> f64;
}
extern "C" {
    pub fn FPDF_GetPageBoundingBox(page: FPDF_PAGE, rect: *mut FS_RECTF) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_GetPageSizeByIndexF(
        document: FPDF_DOCUMENT,
        page_index: ::std::os::raw::c_int,
        size: *mut FS_SIZEF,
    ) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_GetPageSizeByIndex(
        document: FPDF_DOCUMENT,
        page_index: ::std::os::raw::c_int,
        width: *mut f64,
        height: *mut f64,
    ) -> ::std::os::raw::c_int;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FPDF_COLORSCHEME_ {
    pub path_fill_color: FPDF_DWORD,
    pub path_stroke_color: FPDF_DWORD,
    pub text_fill_color: FPDF_DWORD,
    pub text_stroke_color: FPDF_DWORD,
}
pub type FPDF_COLORSCHEME = FPDF_COLORSCHEME_;
extern "C" {
    pub fn FPDF_RenderPage(
        dc: HDC,
        page: FPDF_PAGE,
        start_x: ::std::os::raw::c_int,
        start_y: ::std::os::raw::c_int,
        size_x: ::std::os::raw::c_int,
        size_y: ::std::os::raw::c_int,
        rotate: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_int,
    );
}
extern "C" {
    pub fn FPDF_RenderPageBitmap(
        bitmap: FPDF_BITMAP,
        page: FPDF_PAGE,
        start_x: ::std::os::raw::c_int,
        start_y: ::std::os::raw::c_int,
        size_x: ::std::os::raw::c_int,
        size_y: ::std::os::raw::c_int,
        rotate: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_int,
    );
}
extern "C" {
    pub fn FPDF_RenderPageBitmapWithMatrix(
        bitmap: FPDF_BITMAP,
        page: FPDF_PAGE,
        matrix: *const FS_MATRIX,
        clipping: *const FS_RECTF,
        flags: ::std::os::raw::c_int,
    );
}
extern "C" {
    pub fn FPDF_ClosePage(page: FPDF_PAGE);
}
extern "C" {
    pub fn FPDF_CloseDocument(document: FPDF_DOCUMENT);
}
extern "C" {
    pub fn FPDF_DeviceToPage(
        page: FPDF_PAGE,
        start_x: ::std::os::raw::c_int,
        start_y: ::std::os::raw::c_int,
        size_x: ::std::os::raw::c_int,
        size_y: ::std::os::raw::c_int,
        rotate: ::std::os::raw::c_int,
        device_x: ::std::os::raw::c_int,
        device_y: ::std::os::raw::c_int,
        page_x: *mut f64,
        page_y: *mut f64,
    ) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_PageToDevice(
        page: FPDF_PAGE,
        start_x: ::std::os::raw::c_int,
        start_y: ::std::os::raw::c_int,
        size_x: ::std::os::raw::c_int,
        size_y: ::std::os::raw::c_int,
        rotate: ::std::os::raw::c_int,
        page_x: f64,
        page_y: f64,
        device_x: *mut ::std::os::raw::c_int,
        device_y: *mut ::std::os::raw::c_int,
    ) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDFBitmap_Create(
        width: ::std::os::raw::c_int,
        height: ::std::os::raw::c_int,
        alpha: ::std::os::raw::c_int,
    ) -> FPDF_BITMAP;
}
extern "C" {
    pub fn FPDFBitmap_CreateEx(
        width: ::std::os::raw::c_int,
        height: ::std::os::raw::c_int,
        format: ::std::os::raw::c_int,
        first_scan: *mut ::core::ffi::c_void,
        stride: ::std::os::raw::c_int,
    ) -> FPDF_BITMAP;
}
extern "C" {
    pub fn FPDFBitmap_GetFormat(bitmap: FPDF_BITMAP) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDFBitmap_FillRect(
        bitmap: FPDF_BITMAP,
        left: ::std::os::raw::c_int,
        top: ::std::os::raw::c_int,
        width: ::std::os::raw::c_int,
        height: ::std::os::raw::c_int,
        color: FPDF_DWORD,
    );
}
extern "C" {
    pub fn FPDFBitmap_GetBuffer(bitmap: FPDF_BITMAP) -> *mut ::core::ffi::c_void;
}
extern "C" {
    pub fn FPDFBitmap_GetWidth(bitmap: FPDF_BITMAP) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDFBitmap_GetHeight(bitmap: FPDF_BITMAP) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDFBitmap_GetStride(bitmap: FPDF_BITMAP) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDFBitmap_Destroy(bitmap: FPDF_BITMAP);
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetPrintScaling(document: FPDF_DOCUMENT) -> FPDF_BOOL;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetNumCopies(document: FPDF_DOCUMENT) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetPrintPageRange(document: FPDF_DOCUMENT) -> FPDF_PAGERANGE;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetPrintPageRangeCount(pagerange: FPDF_PAGERANGE) -> usize;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetPrintPageRangeElement(
        pagerange: FPDF_PAGERANGE,
        index: usize,
    ) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetDuplex(document: FPDF_DOCUMENT) -> FPDF_DUPLEXTYPE;
}
extern "C" {
    pub fn FPDF_VIEWERREF_GetName(
        document: FPDF_DOCUMENT,
        key: FPDF_BYTESTRING,
        buffer: *mut ::std::os::raw::c_char,
        length: ::std::os::raw::c_ulong,
    ) -> ::std::os::raw::c_ulong;
}
extern "C" {
    pub fn FPDF_CountNamedDests(document: FPDF_DOCUMENT) -> FPDF_DWORD;
}
extern "C" {
    pub fn FPDF_GetNamedDestByName(document: FPDF_DOCUMENT, name: FPDF_BYTESTRING) -> FPDF_DEST;
}
extern "C" {
    pub fn FPDF_GetNamedDest(
        document: FPDF_DOCUMENT,
        index: ::std::os::raw::c_int,
        buffer: *mut ::core::ffi::c_void,
        buflen: *mut ::std::os::raw::c_long,
    ) -> FPDF_DEST;
}
extern "C" {
    pub fn FPDF_GetXFAPacketCount(document: FPDF_DOCUMENT) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn FPDF_GetXFAPacketName(
        document: FPDF_DOCUMENT,
        index: ::std::os::raw::c_int,
        buffer: *mut ::core::ffi::c_void,
        buflen: ::std::os::raw::c_ulong,
    ) -> ::std::os::raw::c_ulong;
}
extern "C" {
    pub fn FPDF_GetXFAPacketContent(
        document: FPDF_DOCUMENT,
        index: ::std::os::raw::c_int,
        buffer: *mut ::core::ffi::c_void,
        buflen: ::std::os::raw::c_ulong,
        out_buflen: *mut ::std::os::raw::c_ulong,
    ) -> FPDF_BOOL;
}

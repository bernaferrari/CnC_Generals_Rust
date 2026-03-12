
#ifndef SIMPLEPLAYER_H
#define SIMPLEPLAYER_H

#include "wmsdk.h"

//
// we preserve a list of "ready-to-free" list of wave headers for the
// caller to unprepare and free
//
typedef struct WAVEHDR_LIST {
    LPWAVEHDR           pwh;
    struct WAVEHDR_LIST *next;
}   WAVEHDR_LIST;

#define SIMPLE_PLAYER_OPEN_EVENT  _T( "45ab58e0-382e-4d1c-ac50-88a5f9601851" )
#define SIMPLE_PLAYER_CLOSE_EVENT _T( "276095fa-a8e0-48e6-ac61-8b0002345607" )
#define WMAPLAY_EVENT             _T( "9e828a72-64f3-48f0-9de8-13dafd0cbd3a" )
///////////////////////////////////////////////////////////////////////////////
class CSimplePlayer : public IWMReaderCallback
{
public:
    CSimplePlayer( HRESULT* phr );
    ~CSimplePlayer();
    
    virtual HRESULT Play( LPCWSTR pszUrl, DWORD dwSecDuration, HANDLE hCompletionEvent, HRESULT *phrCompletion );

//
// IUnknown Implemenation
//
public:
    virtual HRESULT STDMETHODCALLTYPE QueryInterface(
        REFIID riid,
        void **ppvObject );

    virtual ULONG STDMETHODCALLTYPE AddRef();
    virtual ULONG STDMETHODCALLTYPE Release();

//
// IWMReaderCallback Implemenation
//
public:
    virtual HRESULT STDMETHODCALLTYPE OnSample( 
        /* [in] */ DWORD dwOutputNum,
        /* [in] */ QWORD cnsSampleTime,
        /* [in] */ QWORD cnsSampleDuration,
        /* [in] */ DWORD dwFlags,
        /* [in] */ INSSBuffer __RPC_FAR *pSample,
        /* [in] */ void __RPC_FAR *pvContext);
    
    virtual HRESULT STDMETHODCALLTYPE OnStatus( 
        /* [in] */ WMT_STATUS Status,
        /* [in] */ HRESULT hr,
        /* [in] */ WMT_ATTR_DATATYPE dwType,
        /* [in] */ BYTE __RPC_FAR *pValue,
        /* [in] */ void __RPC_FAR *pvContext);

//
// Helper Methods
//
protected:

    HRESULT Close();

    void OnWaveOutMsg( UINT uMsg, DWORD dwParam1, DWORD dwParam2 );

    static void CALLBACK WaveProc(
                          HWAVEOUT hwo,      
                          UINT uMsg,         
                          DWORD dwInstance,  
                          DWORD dwParam1,    
                          DWORD dwParam2 );
 
    HRESULT AddWaveHeader( LPWAVEHDR pwh );
    void    RemoveWaveHeaders( void );

    CRITICAL_SECTION    m_CriSec;
    WAVEHDR_LIST       *m_whdrHead;

    LONG    m_cRef;
    LONG    m_cBuffersOutstanding;
    BOOL    m_fEof;
    HANDLE  m_hCompletionEvent;

    IWMReader *m_pReader;
    IWMHeaderInfo *m_pHeader;
    HWAVEOUT m_hwo;

    HRESULT *m_phrCompletion;

    HRESULT m_hrOpen;
    HANDLE m_hOpenEvent;
    HANDLE m_hCloseEvent;

    union
    {
        WAVEFORMATEX m_wfx;
        BYTE m_WfxBuf[1024];
    };

    LPWSTR  m_pszUrl;

};

#endif // SIMPLEPLAYER_H

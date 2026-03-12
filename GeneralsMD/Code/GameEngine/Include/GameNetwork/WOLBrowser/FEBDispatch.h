//
// FEBDispatch class is a template class which, when inherited from, can implement the
// IDispatch for a COM object with a type library.
//

#ifndef _FEBDISPATCH_H__
#define _FEBDISPATCH_H__

#include <atlbase.h>
extern CComModule _Module;
#include <atlcom.h>
#include <comutil.h>    // For _bstr_t.

#include "oleauto.h"

template <class T, class C, const IID *I>
class FEBDispatch :
public CComObjectRootEx<CComSingleThreadModel>,
public CComCoClass<T>,
public C
{
public:
	
	BEGIN_COM_MAP(T)
		COM_INTERFACE_ENTRY(C)
		COM_INTERFACE_ENTRY_AGGREGATE(IID_IDispatch, m_dispatch)
	END_COM_MAP()
		
		FEBDispatch()
	{
		m_ptinfo = NULL;
		m_dispatch = NULL;
		
		ITypeLib *ptlib;
		HRESULT hr;
		HRESULT TypeLibraryLoadResult;
		char filename[256];

		GetModuleFileName(NULL, filename, sizeof(filename));
		_bstr_t bstr(filename);
		
		TypeLibraryLoadResult = LoadTypeLib(bstr, &ptlib);

		DEBUG_ASSERTCRASH(TypeLibraryLoadResult == 0, ("Can't load type library for Embedded Browser"));
		
		if (TypeLibraryLoadResult == S_OK)
		{
			hr = ptlib->GetTypeInfoOfGuid(*I, &m_ptinfo);
			ptlib->Release();
			
			if (hr == S_OK)
			{
				hr = CreateStdDispatch(static_cast<IUnknown*>(this), static_cast<C*>(this), m_ptinfo, &m_dispatch);
				
				m_dispatch->AddRef();
				// Don't release the IUnknown from CreateStdDispatch without calling AddRef.
				// It looks like CreateStdDispatch doesn't call AddRef on the IUnknown it returns.
			}
		}
		
		if ( (m_dispatch == NULL) )
		{
			DEBUG_LOG(("Error creating Dispatch for Web interface\n"));
		}
	}
	
	virtual ~FEBDispatch()
	{
		if (m_ptinfo)
			m_ptinfo->Release();
		
		if (m_dispatch)
			m_dispatch->Release();
	}
	
	IUnknown *m_dispatch;

private:
	ITypeInfo *m_ptinfo;
};

#endif

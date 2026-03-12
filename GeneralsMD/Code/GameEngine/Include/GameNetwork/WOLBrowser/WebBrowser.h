/******************************************************************************
*
* NAME
*     $Archive:  $
*
* DESCRIPTION
*     Web Browser
*
* PROGRAMMER
*     Bryan Cleveland
*     $Author:  $
*
* VERSION INFO
*     $Revision:  $
*     $Modtime:  $
*
******************************************************************************/

#pragma once

#ifndef __WEBBROWSER_H__
#define __WEBBROWSER_H__

#include "Common/SubsystemInterface.h"
#include <atlbase.h>
#include <windows.h>
#include <Common/GameMemory.h>
#include "EABrowserDispatch/BrowserDispatch.h"
#include "FEBDispatch.h"

class GameWindow;

class WebBrowserURL : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( WebBrowserURL, "WebBrowserURL" )

public:

	WebBrowserURL();
	// virtual destructor prototype defined by memory pool object

	const FieldParse *getFieldParse( void ) const { return m_URLFieldParseTable; }

	AsciiString m_tag;
	AsciiString m_url;

	WebBrowserURL *m_next;

	static const FieldParse m_URLFieldParseTable[];		///< the parse table for INI definition

};



class WebBrowser :
		public FEBDispatch<WebBrowser, IBrowserDispatch, &IID_IBrowserDispatch>,
		public SubsystemInterface
	{
	public:
		void init( void );
		void reset( void );
		void update( void );

		// Create an instance of the embedded browser for Dune Emperor.
		virtual Bool createBrowserWindow(char *tag, GameWindow *win) = 0;
		virtual void closeBrowserWindow(GameWindow *win) = 0;

		WebBrowserURL *makeNewURL(AsciiString tag);
		WebBrowserURL *findURL(AsciiString tag);

	protected:
		// Protected to prevent direct construction via new, use CreateInstance() instead.
		WebBrowser();
		virtual ~WebBrowser();

		// Protected to prevent copy and assignment
		WebBrowser(const WebBrowser&);
		const WebBrowser& operator=(const WebBrowser&);

//		Bool RetrievePageURL(const char* page, char* url, int size);
//		Bool RetrieveHTMLPath(char* path, int size);

	protected:
		ULONG mRefCount;
		WebBrowserURL *m_urlList;

	//---------------------------------------------------------------------------
	// IUnknown methods
	//---------------------------------------------------------------------------
	protected:
		HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** ppvObject);
		ULONG STDMETHODCALLTYPE AddRef(void);
		ULONG STDMETHODCALLTYPE Release(void);

	//---------------------------------------------------------------------------
	// IBrowserDispatch methods
	//---------------------------------------------------------------------------
	public:
		STDMETHOD(TestMethod)(Int num1);
	};

extern CComObject<WebBrowser> *TheWebBrowser;
#endif // __WEBBROWSER_H__

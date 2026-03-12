/******************************************************************************
*
* NAME
*     $Archive:  $
*
* DESCRIPTION
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

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

//#include "WinMain.h"
#include "GameNetwork/WOLBrowser/WebBrowser.h"
#include "GameClient/GameWindow.h"
#include "GameClient/Display.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

/**
	* OLEInitializer class - Init and shutdown OLE & COM as a global
	* object.  Scary, nasty stuff, COM.  /me shivers.
	*/
class OLEInitializer
{
public:
	OLEInitializer()
	{
		// Initialize this instance
		OleInitialize(NULL);
	 }
	~OLEInitializer()
	{
		OleUninitialize(); 
	}
};
OLEInitializer g_OLEInitializer;
CComModule _Module;

CComObject<WebBrowser> * TheWebBrowser = NULL;


/******************************************************************************
*
* NAME
*     WebBrowser::WebBrowser
*
* DESCRIPTION
*     Default constructor
*
* INPUTS
*     NONE
*
* RESULT
*     NONE
*
******************************************************************************/

WebBrowser::WebBrowser() :
		mRefCount(1)
{
	DEBUG_LOG(("Instantiating embedded WebBrowser\n"));
	m_urlList = NULL;
}


/******************************************************************************
*
* NAME
*     WebBrowser::~WebBrowser
*
* DESCRIPTION
*     Destructor
*
* INPUTS
*     NONE
*
* RESULT
*     NONE
*
******************************************************************************/

WebBrowser::~WebBrowser()
{
	DEBUG_LOG(("Destructing embedded WebBrowser\n"));
	if (this == TheWebBrowser) {
		DEBUG_LOG(("WebBrowser::~WebBrowser - setting TheWebBrowser to NULL\n"));
		TheWebBrowser = NULL;
	}
	WebBrowserURL *url = m_urlList;
	while (url != NULL) {
		WebBrowserURL *temp = url;
		url = url->m_next;
		temp->deleteInstance();
		temp = NULL;
	}
}

//-------------------------------------------------------------------------------------------------
/** The INI data fields for Webpage URL's */
//-------------------------------------------------------------------------------------------------
const FieldParse WebBrowserURL::m_URLFieldParseTable[] = 
{

	{ "URL",										INI::parseAsciiString,							NULL, offsetof( WebBrowserURL, m_url ) },
	{ NULL,											NULL,																NULL, 0 },

};

WebBrowserURL::WebBrowserURL() 
{
	m_next = NULL;
	m_tag.clear();
	m_url.clear();
}

WebBrowserURL::~WebBrowserURL() 
{
}
/******************************************************************************
*
* NAME
*     WebBrowser::init
*
* DESCRIPTION
*     Perform post creation initialization.
*
* INPUTS
*     NONE
*
* RESULT
*     NONE
*
******************************************************************************/

void WebBrowser::init() 
{
	m_urlList = NULL;
	INI ini;
	ini.load( AsciiString( "Data\\INI\\Webpages.ini" ), INI_LOAD_OVERWRITE, NULL );
}

/******************************************************************************
*
* NAME
*     WebBrowser::reset
*
* DESCRIPTION
*     Perform post creation initialization.
*
* INPUTS
*     NONE
*
* RESULT
*     NONE
*
******************************************************************************/

void WebBrowser::reset() 
{
}

void WebBrowser::update( void ) 
{
}

WebBrowserURL * WebBrowser::findURL(AsciiString tag) 
{
	WebBrowserURL *retval = m_urlList;

	while ((retval != NULL) && tag.compareNoCase(retval->m_tag.str())) 
	{
		retval = retval->m_next;
	}

	return retval;
}

WebBrowserURL * WebBrowser::makeNewURL(AsciiString tag) 
{
	WebBrowserURL *newURL = newInstance(WebBrowserURL);

	newURL->m_tag = tag;

	newURL->m_next = m_urlList;
	m_urlList = newURL;

	return newURL;
}

/******************************************************************************
*
* NAME
*     IUnknown::QueryInterface
*
* DESCRIPTION
*
* INPUTS
*     IID - Interface ID
*
* RESULT
*
******************************************************************************/

STDMETHODIMP WebBrowser::QueryInterface(REFIID iid, void** ppv)
{
	*ppv = NULL;

	if ((iid == IID_IUnknown) || (iid == IID_IBrowserDispatch))
	{
		*ppv = static_cast<IBrowserDispatch*>(this);
	}
	else
	{
		return E_NOINTERFACE;
	}

	static_cast<IUnknown*>(*ppv)->AddRef();

	return S_OK;
}


/******************************************************************************
*
* NAME
*     IUnknown::AddRef
*
* DESCRIPTION
*
* INPUTS
*     NONE
*
* RESULT
*
******************************************************************************/

ULONG STDMETHODCALLTYPE WebBrowser::AddRef(void)
{
	return ++mRefCount;
}


/******************************************************************************
*
* NAME
*     IUnknown::Release
*
* DESCRIPTION
*
* INPUTS
*     NONE
*
* RESULT
*
******************************************************************************/

ULONG STDMETHODCALLTYPE WebBrowser::Release(void)
{
	DEBUG_ASSERTCRASH(mRefCount > 0, ("Negative reference count"));
	--mRefCount;

	if (mRefCount == 0)
	{
		DEBUG_LOG(("WebBrowser::Release - all references released, deleting the object.\n"));
		if (this == TheWebBrowser) {
			TheWebBrowser = NULL;
		}
		delete this;
		return 0;
	}

	return mRefCount;
}

STDMETHODIMP WebBrowser::TestMethod(Int num1) 
{
	DEBUG_LOG(("WebBrowser::TestMethod - num1 = %d\n", num1));
	return S_OK;
}

////// W3DWebBrowser.cpp ///////////////
// July 2002 Bryan Cleveland

#include "W3DDevice/GameClient/W3DWebBrowser.h"
#include "WW3D2/Texture.h"
#include "WW3D2/TextureLoader.h"
#include "WW3D2/SurfaceClass.h"
#include "GameClient/Image.h"
#include "GameClient/GameWindow.h"
#include "vector2i.h"
#include <d3dx8.h>
#include "WW3D2/dx8wrapper.h"
#include "WW3D2/dx8WebBrowser.h"

W3DWebBrowser::W3DWebBrowser() : WebBrowser() {
}

Bool W3DWebBrowser::createBrowserWindow(char *tag, GameWindow *win) 
{

	WinInstanceData *winData = win->winGetInstanceData();
	AsciiString windowName = winData->m_decoratedNameString;

	Int x, y, w, h;

	win->winGetSize(&w, &h);
	win->winGetScreenPosition(&x, &y);

	WebBrowserURL *url = findURL( AsciiString(tag) );

	if (url == NULL) {
		DEBUG_LOG(("W3DWebBrowser::createBrowserWindow - couldn't find URL for page %s\n", tag));
		return FALSE;
	}

	CComQIPtr<IDispatch> idisp(m_dispatch);
	if (m_dispatch == NULL)
	{
		return FALSE;
	}

	DX8WebBrowser::CreateBrowser(windowName.str(), url->m_url.str(), x, y, w, h, 0, BROWSEROPTION_SCROLLBARS | BROWSEROPTION_3DBORDER, (LPDISPATCH)this);

	return TRUE;
}

void W3DWebBrowser::closeBrowserWindow(GameWindow *win) 
{
	DX8WebBrowser::DestroyBrowser(win->winGetInstanceData()->m_decoratedNameString.str());
}

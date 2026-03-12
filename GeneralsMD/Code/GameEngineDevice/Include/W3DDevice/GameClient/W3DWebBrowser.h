///// W3DWebBrowser.h ////////////////////////
// July 2002, Bryan Cleveland

#pragma once

#ifndef W3DWEBBROWSER_H
#define W3DWEBBROWSER_H

#include "GameNetwork/WOLBrowser/WebBrowser.h"

class TextureClass;
class Image;
class GameWindow;

class W3DWebBrowser : public WebBrowser
{
	public:
		W3DWebBrowser();

		virtual Bool createBrowserWindow(char *url, GameWindow *win);
		virtual void closeBrowserWindow(GameWindow *win);

};

#endif // #ifndef W3DWEBBROWSER_H
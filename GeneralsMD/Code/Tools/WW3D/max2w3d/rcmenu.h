#ifndef RCMENU_H
#define RCMENU_H

#include "Max.h"
#include "dllmain.h"
#include "resource.h"
#include "istdplug.h"

class W3DUtilityClass;

/**********************************************************************************************
**
** RCMenuClass - W3D Utility's right-click menu.
**
**********************************************************************************************/
class RCMenuClass : public RightClickMenu
{

public:

	RCMenuClass() {Installed=FALSE;}
	~RCMenuClass() {}

	void Bind(Interface * ipi, W3DUtilityClass * eni) { InterfacePtr = ipi; UtilityPtr = eni; }

	void Init(RightClickMenuManager* manager, HWND hWnd, IPoint2 m);
	void Selected(UINT id);
	void Toggle_Hierarchy(INode * node);
	void Toggle_Geometry(INode * node);

public:

	BOOL Installed;

private:

	Interface *				InterfacePtr;
	W3DUtilityClass *		UtilityPtr; 
	INode *					SelNode;
	
	enum {
		MENU_SEPARATOR = 0,
		MENU_TOGGLE_HIERARCHY,
		MENU_TOGGLE_GEOMETRY,
		MENU_NODE_NAME,
		MENU_NODE_POINTER
	};
};

extern RCMenuClass TheRCMenu;

#endif

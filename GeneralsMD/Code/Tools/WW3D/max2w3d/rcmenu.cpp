#include "rcmenu.h"
#include "w3dutil.h"
#include "util.h"


RCMenuClass TheRCMenu;

/*********************************************************************************************** 
 * RCMenuClass::Init -- initialize the "right-click" menu                                      * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void RCMenuClass::Init(RightClickMenuManager* manager, HWND hWnd, IPoint2 m)
{
	Installed=TRUE;

	SelNode = InterfacePtr->PickNode(hWnd,m);

	if (SelNode) {
		
		UINT menuflags;
		W3DAppData2Struct * wdata = W3DAppData2Struct::Get_App_Data(SelNode);

		/*
		** Add the menu separator
		*/
		manager->AddMenu(this, MF_SEPARATOR, MENU_SEPARATOR, NULL);

		/*
		** Add the Name of the object
		*/
		char string[64];
		sprintf(string,"%s:",SelNode->GetName());
		manager->AddMenu(this, MF_STRING | MF_DISABLED, MENU_NODE_NAME, string);

		/*
		** Add the pointer
		*/
//		sprintf(string,"0x%X",(unsigned long)SelNode);
//		manager->AddMenu(this, MF_STRING | MF_GRAYED, MENU_NODE_POINTER, string);

		/*
		** Add the hierarchy menu option
		*/
		if (wdata->Is_Bone()) {
			menuflags = MF_STRING | MF_CHECKED;	
		} else {
			menuflags = MF_STRING;
		}
		manager->AddMenu(this, menuflags, MENU_TOGGLE_HIERARCHY, "W3D: Export Hierarchy");
		
		/*
		** Add the geometry menu option
		*/
		if (wdata->Is_Geometry()) {
			menuflags = MF_STRING | MF_CHECKED;	
		} else {
			menuflags = MF_STRING;
		}
		manager->AddMenu(this, menuflags, MENU_TOGGLE_GEOMETRY, "W3D: Export Geometry");

	}
}

/*********************************************************************************************** 
 * RCMenuClass::Selected -- menu selection callback                                            * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void RCMenuClass::Selected(UINT id)
{	
	switch (id) {

		case MENU_TOGGLE_HIERARCHY:
			Toggle_Hierarchy(SelNode);
			break;
		case MENU_TOGGLE_GEOMETRY:
			Toggle_Geometry(SelNode);
			break;

	}
}

/*********************************************************************************************** 
 * RCMenuClass::Toggle_Hierarchy -- toggle the "export hierarchy" option                       * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void RCMenuClass::Toggle_Hierarchy(INode * node)
{
	
	W3DAppData2Struct * wdata = W3DAppData2Struct::Get_App_Data(SelNode);
	assert(wdata);

	if (wdata->Is_Bone()) {
		wdata->Enable_Export_Transform(false);
	} else {
		wdata->Enable_Export_Transform(true);
	}
}

/*********************************************************************************************** 
 * RCMenuClass::Toggle_Geometry -- toggle the "export geometry" option                         * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void RCMenuClass::Toggle_Geometry(INode * node)
{
	W3DAppData2Struct * wdata = W3DAppData2Struct::Get_App_Data(SelNode);
	assert(wdata);

	if (wdata->Is_Geometry()) {
		wdata->Enable_Export_Geometry(false);
	} else {
		wdata->Enable_Export_Geometry(true);
	}
}



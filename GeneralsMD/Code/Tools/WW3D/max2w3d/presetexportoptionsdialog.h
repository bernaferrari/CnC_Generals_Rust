#ifndef __PRESET_EXPORT_OPTIONS_DIALOG_H
#define __PRESET_EXPORT_OPTIONS_DIALOG_H

#include <windows.h>
#include <max.h>
#include "w3dutil.h"


////////////////////////////////////////////////////////////////////////////////////////
//
//	PresetExportOptionsDialogClass
//
////////////////////////////////////////////////////////////////////////////////////////
class PresetExportOptionsDialogClass
{
public:

	//////////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	//////////////////////////////////////////////////////////////////
	PresetExportOptionsDialogClass (Interface *maxinterface, HWND parent_wnd = NULL);
	~PresetExportOptionsDialogClass (void);


	//////////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////////		
	
	void			Set_Options (W3dExportOptionsStruct *options)	{ Options = options; ::memcpy (&OrigOptions, Options, sizeof (OrigOptions)); }
	int			Do_Modal (void);

private:

	//////////////////////////////////////////////////////////////////
	//	Private data types
	//////////////////////////////////////////////////////////////////
	
	enum
	{
		PANE_HLOD			= 0,
		PANE_ANIM_HLOD,
		PANE_ANIM,
		PANE_TERRAIN,
		PANE_SKELETON,
		PANE_MESH,
		PANE_MAX
	};


	//////////////////////////////////////////////////////////////////
	//	Static methods
	//////////////////////////////////////////////////////////////////
	static BOOL CALLBACK	Real_Message_Proc (HWND wnd, UINT message, WPARAM wparam, LPARAM lparam);
	static BOOL CALLBACK	Settings_Pane_Message_Proc (HWND wnd, UINT message, WPARAM wparam, LPARAM lparam);
	
	//////////////////////////////////////////////////////////////////
	//	Private methods
	//////////////////////////////////////////////////////////////////
	BOOL			Message_Proc (UINT message, WPARAM wparam, LPARAM lparam);
	BOOL			Pane_Message_Proc (UINT message, WPARAM wparam, LPARAM lparam);
	BOOL			Settings_Message_Proc (UINT message, WPARAM wparam, LPARAM lparam);
	BOOL			On_Command (WPARAM wparam, LPARAM lparam);
	void			Show_Settings_Pane (int pane_id);
	void			Create_Settings_Panes (void);
	void			Destroy_Settings_Panes (void);
	void			Determine_Preset_Type (void);
	void			Initialize_Controls (void);
	void			Update_Controls (void);
	void			Save_Settings (void);

	//////////////////////////////////////////////////////////////////
	//	Private member data
	//////////////////////////////////////////////////////////////////
	W3dExportOptionsStruct *	Options;
	W3dExportOptionsStruct		OrigOptions;
	Interface *						MaxInterface;
	HWND								Wnd;
	HWND								ParentWnd;
	HWND								PaneWnds[PANE_MAX];
	int								CurrentPane;
};


#endif //__PRESET_EXPORT_OPTIONS_DIALOG_H


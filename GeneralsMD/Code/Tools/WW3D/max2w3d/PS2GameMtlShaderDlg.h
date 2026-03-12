#ifndef PS2GAMEMTLSHADERDLG_H
#define PS2GAMEMTLSHADERDLG_H

#include <Max.h>
#include "GameMtlForm.h"

// This class was taken from GTH's GameMtlShaderDlg. 

class GameMtl;
struct PS2ShaderBlendSettingPreset;

class PS2GameMtlShaderDlg : public GameMtlFormClass
{

public:

	PS2GameMtlShaderDlg(HWND parent, IMtlParams * imp, GameMtl * m, int pass);

	virtual BOOL		Dialog_Proc (HWND dlg_wnd, UINT message, WPARAM wparam, LPARAM lparam);

	void					ReloadDialog(void);

	// Pure virtual that must be defined.
	void					ActivateDlg(BOOL onOff) {}

private:

	void					Apply_Preset(int preset_index);
	void					Set_Preset(void);
	bool					CompareShaderToBlendPreset(const PS2ShaderBlendSettingPreset &blend_preset);
	void					Set_Advanced_Defaults(void);
};

#endif
#ifndef GAMEMTLSHADERDLG_H
#define GAMEMTLSHADERDLG_H

#include <Max.h>
#include "GameMtlForm.h"


class GameMtl;
struct ShaderBlendSettingPreset;

class GameMtlShaderDlg : public GameMtlFormClass
{

public:

	GameMtlShaderDlg(HWND parent, IMtlParams * imp, GameMtl * m, int pass);
	~GameMtlShaderDlg();

	virtual BOOL		Dialog_Proc (HWND dlg_wnd, UINT message, WPARAM wparam, LPARAM lparam);

	void					ActivateDlg(BOOL onOff);
	void					ReloadDialog(void);

private:

	void					Apply_Preset(int preset_index);
	void					Set_Preset(void);
	bool					CompareShaderToBlendPreset(const ShaderBlendSettingPreset &blend_preset);
	void					Set_Advanced_Defaults(void);
};

#endif
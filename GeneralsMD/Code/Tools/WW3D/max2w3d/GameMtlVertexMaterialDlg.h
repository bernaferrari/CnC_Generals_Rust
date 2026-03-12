#ifndef GAMEMTLVERTEXMATERIALDLG_H
#define GAMEMTLVERTEXMATERIALDLG_H

#include <Max.h>
#include "GameMtlForm.h"

class GameMtl;

class GameMtlVertexMaterialDlg : public GameMtlFormClass
{

public:

	GameMtlVertexMaterialDlg(HWND parent, IMtlParams * imp, GameMtl * m, int pass);
	~GameMtlVertexMaterialDlg();

	virtual BOOL		Dialog_Proc (HWND dlg_wnd, UINT message, WPARAM wparam, LPARAM lparam);

	void					ActivateDlg(BOOL onoff);
	void					ReloadDialog(void);

private:

	enum { MAX_STAGES = 2 };

	IColorSwatch *		AmbientSwatch;
	IColorSwatch *		DiffuseSwatch;
	IColorSwatch *		SpecularSwatch;
	IColorSwatch *		EmissiveSwatch;

	ISpinnerControl * OpacitySpin;
	ISpinnerControl * TranslucencySpin;
	ISpinnerControl * ShininessSpin;
	ISpinnerControl * UVChannelSpin[MAX_STAGES];
};


#endif
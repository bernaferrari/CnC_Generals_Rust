#ifndef GAMEMAPS_H
#define GAMEMAPS_H

#include <Max.h>
#include "stdmat.h"


ClassDesc * Get_Game_Maps_Desc();


///////////////////////////////////////////////////////////////////////////
//
//		TexmapSlotClass
//
///////////////////////////////////////////////////////////////////////////
class TexmapSlotClass
{
public:

	BOOL			MapOn;
	float			Amount;
	Texmap *		Map;

	TexmapSlotClass() : MapOn(FALSE), Amount(1.0f), Map(NULL) {};

	RGBA		Eval(ShadeContext& sc)						{ return Map->EvalColor(sc); 	}
	float		EvalMono(ShadeContext& sc) 				{ return Map->EvalMono(sc); }
	Point3	EvalNormalPerturb(ShadeContext &sc) 	{ return Map->EvalNormalPerturb(sc); }
	BOOL		IsActive() 										{ return (Map && MapOn); }
	void		Update(TimeValue t, Interval &ivalid)	{ if (IsActive()) Map->Update(t,ivalid); };				
	float		GetAmount(TimeValue t) 						{ return Amount; }
};


///////////////////////////////////////////////////////////////////////////
//
//		Texture Maps for In-Game material
//		
//		This class can contain a collection of all of the maps which
//		MAX uses but the GameMtl plugin will only give the user access
//		to the ones we can actually use in the game.
//
///////////////////////////////////////////////////////////////////////////
class GameMapsClass: public ReferenceTarget 
{
public:  

	MtlBase *			Client;
	TexmapSlotClass	TextureSlot[NTEXMAPS];

	GameMapsClass()				 												{ Client = NULL; }
	GameMapsClass(MtlBase *mb)	 												{ Client = mb;	}

	void					DeleteThis()											{ delete this;	}
	void					SetClientPtr(MtlBase *mb)							{ Client = mb; }
	TexmapSlotClass &	operator[](int i) 									{ return TextureSlot[i]; }	
	Class_ID				ClassID();
	SClass_ID			SuperClassID() 										{ return REF_MAKER_CLASS_ID; }
	int					NumSubs()												{ return NTEXMAPS; }
	Animatable * 		SubAnim(int i)											{ return TextureSlot[i].Map; }
	TSTR					SubAnimName(int i)									{ return Client->GetSubTexmapTVName(i); }
	int					NumRefs()												{ return NTEXMAPS; }
	RefTargetHandle	GetReference(int i)									{ return TextureSlot[i].Map; }
	void					SetReference(int i, RefTargetHandle rtarg)	{ TextureSlot[i].Map = (Texmap*)rtarg; }
	int					SubNumToRefNum(int subNum) 						{ return subNum; }


	BOOL					AssignController(Animatable *control,int subAnim);
	RefTargetHandle	Clone(RemapDir &remap);	
	RefResult			NotifyRefChanged( Interval changeInt, RefTargetHandle hTarget, PartID& partID, RefMessage message);

	IOResult				Save(ISave * isave);
	IOResult				Load(ILoad * iload);
};


#endif /*GAMEMAPS_H*/
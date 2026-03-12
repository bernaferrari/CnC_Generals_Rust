#ifndef __MESH_DEFORM_H
#define __MESH_DEFORM_H

#include <Max.h>
#include "Vector.H"

// Forward declarations
class MeshDeformPanelClass;
class MeshDeformModData;


///////////////////////////////////////////////////////////////////////////
//
//	Prototypes
//
///////////////////////////////////////////////////////////////////////////
ClassDesc *Get_Mesh_Deform_Desc (void);
extern Class_ID _MeshDeformClassID;


///////////////////////////////////////////////////////////////////////////
//
//	MeshDeformClass
//
///////////////////////////////////////////////////////////////////////////
class MeshDeformClass : public OSModifier
{
	public:
		
		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		MeshDeformClass (void)
			:	m_MaxInterface (NULL),
				m_ModeMove (NULL),
				m_ModeSelect (NULL),
				m_ModeRotate (NULL),
				m_ModeUScale (NULL),
				m_ModeNUScale (NULL),
				m_ModeSquash (NULL),
				m_DeformState (1.0F),
				m_pPanel (NULL),
				m_CurrentSet (0),
				m_bSetDirty (true),
				m_VertColorChanging (false),
				m_MaxSets (0),
				m_hRollupWnd (NULL)			{ SetName ("WW Mesh Deformer"); Set_Max_Deform_Sets (1); }

		virtual ~MeshDeformClass (void)	{ }
#if defined W3D_MAX4		//defined as in the project (.dsp)
		NumSubObjTypes();
		GetSubObjType();
#endif
		//////////////////////////////////////////////////////////////////////
		//	Public methods
		//////////////////////////////////////////////////////////////////////
		void							Set_Deform_State (float state = 1.0F);
		float							Get_Deform_State (void) const					{ return m_DeformState; }
		void							Set_Vertex_Color (const Point3 &color, bool button_up);
		void							Get_Vertex_Color (Point3 &color);
		void							Set_Max_Deform_Sets (int max);
		int							Get_Max_Deform_Sets (void) const				{ return m_MaxSets; }
		void							Set_Current_Set (int index, bool update_selection);
		int							Get_Current_Set (void) const					{ return m_CurrentSet; }
		void							Update_UI (MeshDeformModData *mod_data);
		void							Auto_Apply (bool auto_apply = true);

		//////////////////////////////////////////////////////////////////////
		//	Base class overrides
		//////////////////////////////////////////////////////////////////////

		//////////////////////////////////////////////////////////////////////
		// From Animatable
		//////////////////////////////////////////////////////////////////////
		void							DeleteThis (void) { delete this; }
		void							GetClassName (TSTR& s) { s = TSTR(_T("WWDeform")); }
		TCHAR *						GetObjectName (void) { return _T("WWDamage"); }
		SClass_ID					SuperClassID (void) { return OSM_CLASS_ID; }		
		Class_ID						ClassID (void) { return _MeshDeformClassID; }
		//RefTargetHandle			Clone(RemapDir& remap = NoRemap());
		void							BeginEditParams (IObjParam  *ip, ULONG flags,Animatable *prev);
		void							EndEditParams (IObjParam *ip, ULONG flags,Animatable *next);
		
		//////////////////////////////////////////////////////////////////////
		// From Modifier
		//////////////////////////////////////////////////////////////////////
		ChannelMask					ChannelsUsed (void);
		ChannelMask					ChannelsChanged (void);
		void							ModifyObject (TimeValue t, ModContext &mod_context, ObjectState* os, INode *node);
		BOOL							DependOnTopology (ModContext &mod_context) { return TRUE; }
		int							NeedUseSubselButton (void) { return TRUE; }
		Class_ID						InputType (void);

		//////////////////////////////////////////////////////////////////////
		// From ReferenceMaker
		//////////////////////////////////////////////////////////////////////
		RefResult					NotifyRefChanged (Interval time, RefTargetHandle htarget, PartID &part_id, RefMessage mesage);
		IOResult						SaveLocalData (ISave *save_obj, LocalModData *mod_context);
		IOResult						LoadLocalData (ILoad *load_obj, LocalModData **mod_context);

		//////////////////////////////////////////////////////////////////////
		// From BaseObject
		//////////////////////////////////////////////////////////////////////
		CreateMouseCallBack *	GetCreateMouseCallBack (void);
		void							ActivateSubobjSel (int level, XFormModes &modes);
		int							HitTest (TimeValue time_value, INode * node, int type, int crossing, int flags, IPoint2 *point, ViewExp *viewport, ModContext *mod_context);
		void							SelectSubComponent (HitRecord *hit_record, BOOL selected, BOOL all, BOOL invert);

		void							GetSubObjectCenters (SubObjAxisCallback *cb, TimeValue t, INode *node, ModContext *mc);
		void							GetSubObjectTMs (SubObjAxisCallback *cb, TimeValue t, INode *node, ModContext *mc);
		int							SubObjectIndex (HitRecord *hitRec) { return hitRec->hitInfo; }
		void							ClearSelection (int selLevel);

		// Transformation managment
		void							Move (TimeValue time_val, Matrix3 &parent_tm, Matrix3 &tm_axis, Point3 &point, BOOL local_origin);
		void							Rotate (TimeValue time_val, Matrix3 &parent_tm, Matrix3 &tm_axis, Quat &rotation, BOOL local_origin);
		void							Scale (TimeValue time_val, Matrix3 &parent_tm, Matrix3 &tm_axis, Point3 &value, BOOL local_origin);

		void							TransformStart (TimeValue time_val);
		void							TransformFinish (TimeValue time_val);
		void							TransformCancel (TimeValue time_val);		
#if defined W3D_MAX4		//defined as in the project (.dsp)
		ISubObjType *				GetSubObjType(int i) ;
#endif

	protected:

		//////////////////////////////////////////////////////////////////////
		//	Protected methods
		//////////////////////////////////////////////////////////////////////
		void							Update_Current_Set (void);
		void							Update_Set_Count (void);
		
	private:

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////		
		IObjParam *					m_MaxInterface;
		HWND							m_hRollupWnd;
		MeshDeformPanelClass *	m_pPanel;
		float							m_DeformState;
		bool							m_VertColorChanging;

		// Mode handlers
		SelectModBoxCMode *		m_ModeSelect;
		MoveModBoxCMode *			m_ModeMove;
		RotateModBoxCMode *		m_ModeRotate;
		UScaleModBoxCMode *		m_ModeUScale;
		NUScaleModBoxCMode *		m_ModeNUScale;
		SquashModBoxCMode *		m_ModeSquash;

		// Set managment
		bool							m_bSetDirty;
		int							m_CurrentSet;
		int							m_MaxSets;

		// Information
		CStr							m_OperationName;
};


#endif //__MESH_DEFORM_H

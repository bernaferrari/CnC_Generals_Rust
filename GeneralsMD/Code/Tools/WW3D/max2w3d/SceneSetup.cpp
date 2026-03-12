/*
** SceneSetup.cpp - Implements the "wwSceneSetup" MAXScript function to
** present a nice dialog to the user for getting a number of parameters
** that will governs the number, placement, and type of LOD and Damage
** models created in the scene.
*/


#include "SceneSetupDlg.h"

#undef STRICT
#include <MaxScrpt.h>
#include <Numbers.h>
#include <Arrays.h>
#include <definsfn.h>


/*
** Let MAXScript know we're implementing new built-in functions.
*/
def_visible_primitive(scene_setup, "wwSceneSetup");


/***********************************************************************************************
 * scene_setup_cf - MAXScript function wwSceneSetup                                            *
 *                                                                                             *
 * wwSceneSetup - Usage: wwSceneSetup arg_array                                                *
 *                                                                                             *
 * INPUT:                                                                                      *
 *	The contents of the given array is assumed to be as follows:                                *
 *		lod_count (int)			- the number of LOD models that will be created                  *
 *		lod_offset (float)		- X offset to move the LODs by                                   *
 *		lod_clone (int)			- 1==copy 2==instance 3==reference                               *
 *		damage_count (int)		- the number of damage models that will be created               *
 *		damage_offset (float)	- Y offset to move the LODs by                                   *
 *		damage_clone (int)		- 1==copy 2==instance 3==reference                               *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 * The function returns an array of the new values the user selected in the same format as     *
 * above.
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   9/27/1999  AJA : Created.                                                                 *
 *=============================================================================================*/
Value * scene_setup_cf (Value **arg_list, int count)
{
	// We don't want any arguments for this function.
	check_arg_count("wwSceneSetup", 1, count);
	type_check(arg_list[0], Array, "Parameter array");

	SceneSetupDlg	dlg(MAXScript_interface);
	one_typed_value_local(Array* result);

	// Read the initial values out of the array.
	Array *args = (Array*)(arg_list[0]);
	dlg.m_LodCount = (args->get(1))->to_int();
	dlg.m_LodOffset = (args->get(2))->to_float();
	dlg.m_LodProc = (args->get(3))->to_int();
	dlg.m_DamageCount = (args->get(4))->to_int();
	dlg.m_DamageOffset = (args->get(5))->to_float();
	dlg.m_DamageProc = (args->get(6))->to_int();

	// Display the dialog
	if (dlg.DoModal() == IDCANCEL)
	{
		pop_value_locals();
		return &undefined;
	}

	// Stuff the values the user chose into the array we'll return.
	vl.result = new Array(6);
	vl.result->append(Integer::intern(dlg.m_LodCount));
	vl.result->append(Float::intern(dlg.m_LodOffset));
	vl.result->append(Integer::intern(dlg.m_LodProc));
	vl.result->append(Integer::intern(dlg.m_DamageCount));
	vl.result->append(Float::intern(dlg.m_DamageOffset));
	vl.result->append(Integer::intern(dlg.m_DamageProc));

	// Return the array of new values.
	return_value(vl.result);
}



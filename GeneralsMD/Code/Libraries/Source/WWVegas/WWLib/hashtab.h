#ifndef HASHTAB_H
#define HASHTAB_H

#if 0

template <class Object,class Key> class NamedObjectHashTableClass
{

public:

	HashTableClass(int initialsize,int growthrate,HashCalculatorClass<Key> * hasher);
	~HashTableClass(void);

	void				Add(Object * new_item,Key * key);
	void				Remove(Object * item,Key * key);

	int				Count(void) const								{ return Items.Count(); }
	Object *			Find(const Key & key) const						

private:

	enum { NO_ITEM = 0xFFFFFFFF };

	class HashItem
	{
	public:
		T * 	Item;
		int	NextHashIndex;

		bool operator == (const HashItem & that) { return ((Item == that.Item) && (NextHashIndex == that.NextHashIndex)); }
		bool operator != (const HashItem & that) { return !(*this == that); }
	};
		
	// Dynamic Vector of the unique items:
	DynamicVectorClass<HashItem>		Items;

	// Hash table:
	int										HashTableSize;
	int *										HashTable;

	// object which does the hashing for the type
	HashCalculatorClass<T> *			HashCalculator;

	friend class VectorClass;
	friend class DynamicVectorClass;
};



template <class Object,class Key>
HashTableClass<Object,Key>::HashTableClass
(
	int initialsize,
	int growthrate,
	HashCalculatorClass<Object> * hasher
)
{

}

template <class Object,class Key>
HashTableClass<Object,Key>::~HashTableClass(void)
{

}

template <class Object,class Key>
Object *	HashTableClass<Object,Key>::Find(Key * key) const
{
	// compute where in the hash table this key would go.
	HashCalculator->Compute_Hash(key);
	int hashval = HashCalculator->Get_Hash_Value(0);

	// now try to find an object which has the same key
	int test_item_index = HashTable[hash];
	while (test_item_index != 0xFFFFFFFF) {
		if (HashCalculator->Items_Match(Items[test_item_index].Item,new_item)) {
			return Items[test_item_index].Object;
		}
		test_item_index = Items[test_item_index].NextHashIndex;
	}

	// couldn't find it
	return NULL;
}

#endif


#endif


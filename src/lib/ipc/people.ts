import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { personSchema, type Person, type PersonRole } from "@/types/person";

export async function listPeople(): Promise<Person[]> {
  const result = await invoke("people_list");
  return z.array(personSchema).parse(result);
}

export interface NewPersonInput {
  canonicalName: string;
  role: PersonRole;
}

export async function createPerson(input: NewPersonInput): Promise<Person> {
  const result = await invoke("people_create", { input });
  return personSchema.parse(result);
}

export async function deletePerson(id: string): Promise<void> {
  await invoke("people_delete", { id });
}

const peoplePolicySchema = z.object({ twitchAutoLearnEnabled: z.boolean(), discordAutoLearnEnabled: z.boolean() });
export type PeoplePolicy = z.infer<typeof peoplePolicySchema>;

export async function getPeoplePolicy(): Promise<PeoplePolicy> {
  const result = await invoke("people_get_policy");
  return peoplePolicySchema.parse(result);
}

export async function setPeoplePolicy(policy: PeoplePolicy): Promise<PeoplePolicy> {
  const result = await invoke("people_set_policy", { policy });
  return peoplePolicySchema.parse(result);
}
